#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Focused bootstrap packet gate; root in a disposable Linux VM only.

All addresses below exist only in a private network namespace. No provider
traffic, host firewall changes or account credentials are used.
"""
import json
import os
from pathlib import Path
import subprocess
import sys


def run(*args, data="", check=True):
    return subprocess.run(args, input=data, text=True, capture_output=True, check=check)


def main(binary):
    run("mount", "--make-rprivate", "/")
    run("mount", "-t", "tmpfs", "tmpfs", "/run")
    Path("/run/upper").mkdir()
    Path("/run/work").mkdir()
    run("mount", "-t", "overlay", "overlay", "-o",
        "lowerdir=/etc,upperdir=/run/upper,workdir=/run/work", "/etc")
    config = Path("/etc/wave-vpn")
    config.mkdir(exist_ok=True)
    run("mount", "-t", "tmpfs", "-o", "mode=700", "tmpfs", str(config))
    (config / "service.json").write_text('{"uid":65534}')
    (config / "service.json").chmod(0o600)
    run("ip", "link", "set", "lo", "up")
    run("ip", "netns", "add", "peer")
    run("ip", "link", "add", "test0", "type", "veth", "peer", "name", "test1")
    run("ip", "link", "set", "test1", "netns", "peer")
    for prefix in [[], ["-n", "peer"]]:
        dev = "test1" if prefix else "test0"
        run("ip", *prefix, "link", "set", dev, "up")
        for address in (["8.8.8.8/24", "9.9.9.9/24", "2606:4700::1111/64"]
                        if prefix else ["8.8.8.7/24", "9.9.9.8/24", "2606:4700::1110/64"]):
            run("ip", *prefix, "address", "add", address, "dev", dev,
                *(["nodad"] if ":" in address else []))
    code = '''import socket,threading,time
def serve(ip,port):
 s=socket.socket(socket.AF_INET6 if ':' in ip else socket.AF_INET)
 s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);s.bind((ip,port));s.listen()
 while True:
  c,a=s.accept();c.sendall(b'ok');c.close()
for ip,port in [('8.8.8.8',4443),('9.9.9.9',8443),('2606:4700::1111',9443)]:
 threading.Thread(target=serve,args=(ip,port),daemon=True).start()
while True: time.sleep(60)
'''
    listener = subprocess.Popen(["ip", "netns", "exec", "peer", sys.executable, "-c", code])

    def probe(ip, port, uid=65534):
        code = ("import socket;s=socket.socket(" +
                ("socket.AF_INET6" if ":" in ip else "socket.AF_INET") +
                f");s.settimeout(.5);s.connect(({ip!r},{port}));assert s.recv(2)==b'ok'")
        return run("setpriv", f"--reuid={uid}", "--regid=65534", "--clear-groups",
                   sys.executable, "-c", code, check=False).returncode == 0

    def action(name, pairs, check=True):
        return run(binary, name, data=json.dumps({"options": {"allow_lan": "false", "allow_ping": "false"},
                   "bootstrap": [{"ip": ip, "port": port} for ip, port in pairs]}), check=check)

    import time
    time.sleep(.5)
    old = [("8.8.8.8", 4443)]
    new = [("9.9.9.9", 8443), ("2606:4700::1111", 9443)]
    try:
        assert all(probe(ip, port) for ip, port in old + new), "listeners unavailable"
        action("lock-enable", old)
        assert probe(*old[0]) and not probe(*old[0], uid=65533)
        assert not any(probe(ip, port) for ip, port in new)
        print("PASS real nft skuid permits authorized UID and blocks another UID", flush=True)
        action("lock-update", new)
        assert not probe(*old[0])
        for ip, port in new:
            assert probe(ip, port) and not probe(ip, port, uid=65533)
            assert not probe(ip, port + 1)
        print("PASS literal IPv4/IPv6 and exact ports rotate without retaining old destination", flush=True)
        before = run("nft", "list", "table", "inet", "wave_vpn").stdout
        # A real rejected kernel nft transaction must leave the original table.
        rejected = run("nft", "-f", "-", data=(
            "delete table inet wave_vpn\n"
            "table inet wave_vpn {\n chain output {\n"
            " type filter hook output priority -10; policy drop;\n }\n}\n"
            "delete table inet wave_test_nonexistent\n"), check=False)
        assert rejected.returncode != 0
        assert "No such file or directory" in rejected.stderr, rejected.stderr
        assert run("nft", "list", "table", "inet", "wave_vpn").stdout == before
        # A helper-side operation-lock conflict must also be retryable.
        import fcntl
        with open("/run/wave-vpn/worker.lock", "a") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            failed = action("lock-update", old, check=False)
        assert failed.returncode != 0, "operation conflict unexpectedly accepted"
        assert run("nft", "list", "table", "inet", "wave_vpn").stdout == before
        action("lock-update", old)
        assert probe(*old[0]) and not probe(*new[0])
        print("PASS rejected nft replacement retains table; failed helper update retries successfully", flush=True)
        run(binary, "recover")
        assert probe(*old[0]) and not probe(*old[0], uid=65533)
        action("lock-update", [])
        assert not probe(*old[0])
        action("lock-disable", [])
        assert all(probe(ip, port) for ip, port in old + new)
        print("PASS Recover retains scoped exceptions; empty update clears; disable restores traffic", flush=True)
    finally:
        listener.terminate()
        listener.wait(timeout=5)


if __name__ == "__main__":
    assert os.geteuid() == 0, "run as root inside a disposable Linux VM"
    binary = str(Path(sys.argv[1]).resolve())
    if len(sys.argv) == 2:
        os.execvp("unshare", ["unshare", "--mount", "--net", "--pid", "--fork", "--mount-proc",
                             sys.executable, str(Path(__file__).resolve()), binary, "--isolated"])
    main(binary)
