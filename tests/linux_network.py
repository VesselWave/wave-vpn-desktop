#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Destructive networking checks, contained in new mount/network/PID namespaces.
Run as root in a Linux test VM: python3 tests/linux_network.py target/debug/wave-vpn
No account credentials, internet connection, or archived test code required.
"""
import base64
import json
import os
from pathlib import Path
import queue
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time


def run(*args, data="", check=True):
    return subprocess.run(args, input=data, text=True, capture_output=True, check=check).stdout


def http(address, namespace=None):
    cmd = ["curl", "--noproxy", "*", "--max-time", "2", "--fail", "-s", f"http://{address}:18080/"]
    if namespace:
        cmd = ["ip", "netns", "exec", namespace] + cmd
    return subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0


def worker_command(binary, action):
    if os.environ.get("WAVE_TEST_SERVICE") == "1" and action != "lock-restore":
        return [binary, "service-client", action]
    return [binary, action]


def main(binary):
    resolver_target = Path("/etc/resolv.conf").resolve()
    run("mount", "--make-rprivate", "/")
    run("mount", "-t", "tmpfs", "-o", "mode=755", "tmpfs", "/run")
    # Config writes go to a private overlay; no host /etc files are changed.
    Path("/run/etc-upper").mkdir()
    Path("/run/etc-work").mkdir()
    run("mount", "-t", "overlay", "overlay", "-o", "lowerdir=/etc,upperdir=/run/etc-upper,workdir=/run/etc-work", "/etc")
    run("ip", "link", "set", "lo", "up")
    run("ip", "netns", "add", "peer")
    run("ip", "link", "add", "physical0", "type", "veth", "peer", "name", "physical1")
    run("ip", "link", "set", "physical1", "netns", "peer")
    run("ip", "address", "add", "198.18.0.1/24", "dev", "physical0")
    run("ip", "address", "add", "192.168.78.1/24", "dev", "physical0")
    run("ip", "-6", "address", "add", "2001:db8:77::1/64", "dev", "physical0", "nodad")
    run("ip", "link", "set", "physical0", "up")
    run("ip", "route", "add", "default", "via", "198.18.0.2")
    for cmd in [["link", "set", "lo", "up"], ["address", "add", "198.18.0.2/24", "dev", "physical1"], ["link", "set", "physical1", "up"]]:
        run("ip", "-n", "peer", *cmd)
    run("ip", "-n", "peer", "address", "add", "192.168.78.2/24", "dev", "physical1")
    run("ip", "-n", "peer", "-6", "address", "add", "2001:db8:77::2/64", "dev", "physical1", "nodad")
    run("ip", "-6", "route", "add", "default", "via", "2001:db8:77::2")
    # /etc is already a private overlay. Keep a regular resolv.conf renameable:
    # bind-mounting over it would make the file backend fail with EBUSY.
    # Symlink targets outside /etc still need a private backing file.
    resolver = Path("/etc/resolv.conf") if resolver_target == Path("/etc/resolv.conf") else Path("/run/test-resolv.conf")
    resolver.write_text("nameserver 203.0.113.53\n")
    resolver.chmod(0o644)
    original_resolver = resolver.read_bytes()
    if not resolver_target.exists():
        assert str(resolver_target).startswith("/run/")
        resolver_target.parent.mkdir(parents=True, exist_ok=True)
        resolver_target.touch()
    if resolver != resolver_target:
        run("mount", "--bind", str(resolver), str(resolver_target))
    # Config lookup is private too, even if the VM has an installed persistent lock.
    if Path("/etc/wave-vpn").exists():
        run("mount", "-t", "tmpfs", "tmpfs", "/etc/wave-vpn")
    service = None
    if os.environ.get("WAVE_TEST_SERVICE") == "1":
        import shutil
        helper = "/run/wave-service-test-helper"
        shutil.copyfile(binary, helper)
        os.chmod(helper, 0o755)
        authorization = Path("/etc/wave-vpn")
        authorization.mkdir(exist_ok=True)
        authorization.chmod(0o700)
        (authorization / "service.json").write_text(json.dumps({"uid": 65534}))
        (authorization / "service.json").chmod(0o600)
        listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        listener.bind("/run/wave-vpn-service.sock")
        listener.listen()
        os.chmod("/run/wave-vpn-service.sock", 0o666)
        launcher = "import os,sys; os.dup2(int(sys.argv[2]),3); os.set_inheritable(3,True); os.environ.update(LISTEN_PID=str(os.getpid()),LISTEN_FDS='1'); os.execv(sys.argv[1],[sys.argv[1],'service-serve'])"
        service = subprocess.Popen([sys.executable, "-c", launcher, helper, str(listener.fileno())], pass_fds=(listener.fileno(),))
        time.sleep(.3)
        assert service.poll() is None, "isolated networking service failed to start"
        print("PASS isolated socket-activated broker started with a root-owned helper", flush=True)
    server_key = run("wg", "genkey").strip()
    client_key = run("wg", "genkey").strip()
    server_pub = run("wg", "pubkey", data=server_key).strip()
    client_pub = run("wg", "pubkey", data=client_key).strip()
    psk = run("wg", "genpsk").strip()
    run("ip", "-n", "peer", "link", "add", "wgpeer0", "type", "wireguard")
    conf = Path("/run/test-peer.conf")
    conf.write_text(f"[Interface]\nPrivateKey = {server_key}\nListenPort = 51830\n[Peer]\nPublicKey = {client_pub}\nPresharedKey = {psk}\nAllowedIPs = 10.77.0.2/32, fd77::2/128\n")
    conf.chmod(0o600)
    run("ip", "netns", "exec", "peer", "wg", "setconf", "wgpeer0", str(conf))
    conf.unlink()
    run("ip", "-n", "peer", "address", "add", "10.77.0.1/24", "dev", "wgpeer0")
    run("ip", "-n", "peer", "-6", "address", "add", "fd77::1/64", "dev", "wgpeer0")
    run("ip", "-n", "peer", "link", "set", "wgpeer0", "up")
    server = subprocess.Popen(["ip", "netns", "exec", "peer", "python3", "-m", "http.server", "18080", "--bind", "::"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    run("nft", "-f", "-", data='table inet unrelated { comment "must survive Wave"; }\n')
    # Echo listeners observe whether DNS packets reach a physical/LAN resolver.
    # They deliberately avoid requiring any external DNS infrastructure.
    dns_code = r"""
import socket,threading,time

def udp():
    s=socket.socket(socket.AF_INET6,socket.SOCK_DGRAM)
    s.setsockopt(socket.IPPROTO_IPV6,socket.IPV6_V6ONLY,0)
    s.bind(('::',53))
    while True:
        b,a=s.recvfrom(512); s.sendto(b,a)
def tcp():
    s=socket.socket(socket.AF_INET6,socket.SOCK_STREAM)
    s.setsockopt(socket.IPPROTO_IPV6,socket.IPV6_V6ONLY,0)
    s.bind(('::',53)); s.listen()
    while True:
        c,a=s.accept(); c.close()
threading.Thread(target=udp,daemon=True).start()
threading.Thread(target=tcp,daemon=True).start()
while True: time.sleep(60)
"""
    dns_server = subprocess.Popen(["ip", "netns", "exec", "peer", "python3", "-c", dns_code])
    def dns_probe(ip, tcp=False, physical=False):
        family = socket.AF_INET6 if ':' in ip else socket.AF_INET
        with socket.socket(family, socket.SOCK_STREAM if tcp else socket.SOCK_DGRAM) as sock:
            sock.settimeout(.4)
            if physical:
                sock.setsockopt(socket.SOL_SOCKET, socket.SO_BINDTODEVICE, b"physical0\0")
            try:
                sock.connect((ip, 53))
                if tcp:
                    return True
                sock.send(b'wave-leak-probe')
                return sock.recv(512) == b'wave-leak-probe'
            except OSError:
                return False

    def blocked_dns():
        for ip in ["192.168.78.2", "2001:db8:77::2"]:
            for tcp in [False, True]:
                assert not dns_probe(ip, tcp, physical=True), f"DNS leak to {ip} tcp={tcp}"

    time.sleep(1.2)
    for ip in ["192.168.78.2", "2001:db8:77::2"]:
        for tcp in [False, True]:
            assert dns_probe(ip, tcp), f"DNS observation listener unavailable: {ip}"

    assert http("198.18.0.2"), "physical test server unreachable before connecting"
    request = {"profile": {"private_key": client_key, "preshared_key": psk, "public_key": server_pub, "addresses": ["10.77.0.2/32", "fd77::2/128"], "dns": ["10.77.0.1"], "endpoint": "198.18.0.2", "port": 51830}, "options": {"netlock": "true", "ipv6": "in-block", "dns_mode": "resolv.conf", "allow_lan": "false", "allow_ping": "false", "verify": "true", "keepalive": "1", "handshake_first": "5"}}

    def worker(req):
        proc = subprocess.Popen(worker_command(binary, "transport-worker"), stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        proc.stdin.write(json.dumps(req) + "\n")
        proc.stdin.flush()
        events = queue.Queue()
        def read():
            for line in proc.stdout:
                try:
                    events.put(json.loads(line))
                except ValueError:
                    pass
            events.put({"exited": True})
        threading.Thread(target=read, daemon=True).start()
        return proc, events

    def connected(proc, events):
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            event = events.get(timeout=12)
            if event.get("event") == "connected":
                return
            if "exited" in event:
                raise AssertionError("worker exited: " + proc.stderr.read())
        raise AssertionError("connection timeout")

    def disconnect(proc):
        proc.stdin.write("disconnect\n")
        proc.stdin.flush()
        proc.stdin.close()

    def worker_pid(proc):
        if service is None:
            return proc.pid
        children = set()
        for task in Path(f"/proc/{service.pid}/task").iterdir():
            children.update((task / "children").read_text().split())
        workers = [int(pid) for pid in children if b"transport-worker" in Path(f"/proc/{pid}/cmdline").read_bytes()]
        assert len(workers) == 1, "cannot identify broker's active worker"
        return workers[0]

    def stopped(proc, success=True):
        code = proc.wait(timeout=12)
        error = proc.stderr.read()
        if success:
            assert code == 0, error
        # A killed client closes its socket; the broker's worker then cleans up.
        for _ in range(100):
            if not Path("/run/wave-vpn/recovery.json").exists():
                break
            time.sleep(.05)
        assert "wavevpn0" not in run("ip", "link", "show"), "tunnel left behind"
        assert "51820" not in run("ip", "rule", "show"), "routing rules left behind"
        assert resolver.read_bytes() == original_resolver, "DNS not restored"
        assert "must survive Wave" in run("nft", "list", "table", "inet", "unrelated")

    proc, events = worker(request)
    connected(proc, events)
    assert http("10.77.0.1"), "tunnel cannot carry traffic"
    assert http("[fd77::1]"), "IPv6 tunnel cannot carry traffic"
    assert not http("[2001:db8:77::2]"), "IPv6 plaintext bypass escaped"
    assert dns_probe("10.77.0.1") and dns_probe("fd77::1"), "tunnel DNS cannot carry packets"
    blocked_dns()
    assert "10.77.0.1" in Path("/etc/resolv.conf").read_text(), "tunnel DNS not configured"
    assert not http("198.18.0.2"), "plaintext bypass escaped the kill switch"
    assert "dev wavevpn0" in run("ip", "route", "get", "1.1.1.1")
    assert "dev wavevpn0" in run("ip", "-6", "route", "get", "2606:4700:4700::1111")
    deadline = time.monotonic() + 5
    received = False
    while time.monotonic() < deadline:
        event = events.get(timeout=5)
        if event.get("event") == "stats" and event.get("rx", 0) > 0:
            received = True
            break
    assert received, "no received-byte statistics"
    disconnect(proc)
    stopped(proc)
    assert not http("198.18.0.2"), "Disconnect released Network Lock"
    assert not http("[2001:db8:77::2]"), "Disconnect released IPv6 Network Lock"
    blocked_dns()
    run(*worker_command(binary, "recover"))
    assert not http("198.18.0.2"), "Recover released Network Lock"
    run(*worker_command(binary, "lock-disable"))
    assert http("198.18.0.2"), "explicit disable did not restore networking"
    print("PASS dual-stack handshake, tunnel traffic/stats, blocked bypass, disconnect/Recover retain IPv4/IPv6/DNS protection; explicit disable restores internet", flush=True)

    # U2404 B-22: a resolver manager (NetworkManager with rc-manager=file)
    # rewrites resolv.conf mid-session. Wave holds NetworkManager off with a
    # runtime drop-in, restores the tunnel resolver within seconds, keeps DNS
    # working under Network Lock, and Recover never wedges on a foreign file.
    drop_in = Path("/run/NetworkManager/conf.d/zz-wave-vpn-dns.conf")
    has_nm = any(Path(d, "NetworkManager").is_file() for d in ["/usr/sbin", "/usr/bin", "/sbin", "/bin"])
    etc_link = os.readlink("/etc/resolv.conf") if Path("/etc/resolv.conf").is_symlink() else None

    def foreign_write(text):
        temporary = Path("/etc/resolv.conf.foreign")
        temporary.write_text(text)
        temporary.chmod(0o644)
        os.replace(temporary, "/etc/resolv.conf")

    def tunnel_resolver(timeout=5):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if "nameserver 10.77.0.1" in Path("/etc/resolv.conf").read_text():
                return True
            time.sleep(.1)
        return False

    foreign = "# Generated by NetworkManager\nsearch lan\nnameserver 192.168.78.2\n"
    proc, events = worker(request)
    connected(proc, events)
    assert drop_in.exists() == has_nm, "NetworkManager runtime drop-in"
    for _ in range(3):
        foreign_write(foreign)
        assert tunnel_resolver(), "foreign resolv.conf was not replaced by the tunnel resolver"
    assert dns_probe("10.77.0.1"), "tunnel DNS broken after a foreign rewrite under Network Lock"
    blocked_dns()
    disconnect(proc)
    stopped(proc)
    assert not drop_in.exists(), "NetworkManager drop-in left behind"
    assert not Path("/etc/resolv.conf.wave-vpn-backup").exists()
    proc, events = worker(request)
    connected(proc, events)
    os.kill(worker_pid(proc), signal.SIGKILL)
    proc.wait(timeout=3)
    time.sleep(1)
    foreign_write(foreign)
    run(*worker_command(binary, "recover"))
    assert not Path("/etc/resolv.conf.wave-vpn-backup").exists(), "Recover kept a stale backup"
    assert Path("/etc/resolv.conf").read_text() == foreign, "Recover discarded the newer resolver"
    assert not drop_in.exists(), "NetworkManager drop-in left behind after Recover"
    os.unlink("/etc/resolv.conf")
    if etc_link:
        os.symlink(etc_link, "/etc/resolv.conf")
    else:
        resolver.write_bytes(original_resolver)
        resolver.chmod(0o644)
    assert resolver.read_bytes() == original_resolver
    run(*worker_command(binary, "lock-disable"))
    assert http("198.18.0.2")
    print("PASS foreign resolv.conf rewrites are undone during a session; Recover keeps a newer foreign resolver", flush=True)

    # U2404 B-14: `nft flush ruleset` while connected and while a retained
    # lock is idle. The worker restores its table itself; idle, the helper
    # reports the loss (lock_lost) and Recover keeps the lock.
    def flush():
        run("nft", "flush", "ruleset")
        run("nft", "-f", "-", data='table inet unrelated { comment "must survive Wave"; }\n')

    proc, events = worker(request)
    connected(proc, events)
    flush()
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline and "wave_vpn" not in run("nft", "list", "tables"):
        time.sleep(.2)
    assert "wave_vpn" in run("nft", "list", "tables"), "worker did not restore its flushed table"
    assert not http("198.18.0.2"), "restored table does not block plaintext"
    assert http("10.77.0.1"), "tunnel broken after the table was restored"
    disconnect(proc)
    stopped(proc)
    flush()
    status_output = run(*worker_command(binary, "network-status"))
    assert '"lock_lost"' in status_output, "flushed retained lock not reported"
    assert http("198.18.0.2")
    run(*worker_command(binary, "recover"))
    assert not http("198.18.0.2"), "Recover dropped a lock another program removed"
    assert '"lock_lost"' not in run(*worker_command(binary, "network-status"))
    run(*worker_command(binary, "lock-disable"))
    assert '"lock_lost"' not in run(*worker_command(binary, "network-status"))
    assert http("198.18.0.2")
    print("PASS a flushed ruleset is restored while connected, reported while idle and kept by Recover", flush=True)

    def actual_status():
        output = run(*worker_command(binary, "network-status"))
        return next(json.loads(line)["status"] for line in output.splitlines() if line.startswith('{') and json.loads(line).get("event") == "network_status")

    assert actual_status()["enabled"] is False
    Path("/run/wave-vpn-protection.json").write_text('{"enabled":true}')
    assert actual_status()["enabled"] is False, "cached preference mistaken for actual lock"
    proc, events = worker(request)
    connected(proc, events)
    assert actual_status()["enabled"] is True and actual_status()["worker_active"]
    Path("/run/wave-vpn-protection.json").unlink()
    assert actual_status()["enabled"] is True, "missing cached state hid actual lock"
    os.kill(worker_pid(proc), signal.SIGKILL)
    proc.wait(timeout=3)
    assert actual_status()["recovery_needed"] and not actual_status()["worker_active"]
    # The next worker must recover the stale resolver/routes itself, retaining lock.
    proc, events = worker(request)
    connected(proc, events)
    assert http("10.77.0.1")
    disconnect(proc)
    stopped(proc)
    assert not actual_status()["recovery_needed"] and actual_status()["enabled"]
    run(*worker_command(binary, "lock-disable"))
    run("nft", "-f", "-", data='table inet wave_vpn { comment "Foreign owner"; }')
    assert actual_status()["enabled"] is None, "foreign table reported as confirmed protection"
    conflict = subprocess.run(worker_command(binary, "lock-enable"), input="", capture_output=True, text=True, timeout=10)
    assert conflict.returncode != 0 and 'Foreign owner' in run("nft", "list", "table", "inet", "wave_vpn")
    run("nft", "delete", "table", "inet", "wave_vpn")
    print("PASS live firewall status ignores stale/missing cache, detects foreign tables; reconnect repairs crash journal without unlocking", flush=True)
    proc, events = worker(request)
    connected(proc, events)
    run("ip", "link", "set", "physical0", "down")
    blocked_dns()
    run("ip", "link", "set", "physical0", "up")
    # Linux can remove IPv6 addresses/routes when an interface goes down.
    run("ip", "-6", "address", "replace", "2001:db8:77::1/64", "dev", "physical0", "nodad")
    run("ip", "-6", "route", "replace", "default", "via", "2001:db8:77::2")
    assert http("10.77.0.1"), "tunnel did not survive a short uplink change"
    os.kill(worker_pid(proc), signal.SIGSTOP)
    time.sleep(16)
    os.kill(worker_pid(proc), signal.SIGCONT)
    stopped(proc, success=False)
    assert not http("198.18.0.2"), "event-loop pause released protection"
    blocked_dns()
    run(*worker_command(binary, "lock-disable"))
    print("PASS uplink down/up and long worker pause detection; failure retains protection (pause simulates resume, not physical suspend)", flush=True)
    if os.environ.get("WAVE_TEST_FOCUS") == "lifecycle":
        server.terminate()
        dns_server.terminate()
        return


    lan_allowed = json.loads(json.dumps(request))
    lan_allowed["options"]["allow_lan"] = "true"
    proc, events = worker(lan_allowed)
    connected(proc, events)
    assert http("192.168.78.2"), "LAN exception not applied"
    blocked_dns()
    disconnect(proc)
    stopped(proc)
    print("PASS LAN access does not allow TCP/UDP DNS fallback outside the tunnel", flush=True)

    routed = json.loads(json.dumps(request))
    routed["options"].update({"routes": "198.18.0.2/32,out", "entry_interface": "physical0", "dns_mode": "auto"})
    proc, events = worker(routed)
    connected(proc, events)
    assert http("198.18.0.2"), "explicit outside route blocked"
    assert http("10.77.0.1"), "custom route broke the tunnel"
    disconnect(proc)
    stopped(proc)
    print("PASS explicit bypass route, entry interface, disconnect cleanup", flush=True)

    run("ip", "netns", "add", "lan")
    run("ip", "link", "add", "lan0", "type", "veth", "peer", "name", "lan1")
    run("ip", "link", "set", "lan1", "netns", "lan")
    run("ip", "address", "add", "192.168.50.1/24", "dev", "lan0")
    run("ip", "link", "set", "lan0", "up")
    run("ip", "-n", "lan", "link", "set", "lo", "up")
    run("ip", "-n", "lan", "address", "add", "192.168.50.2/24", "dev", "lan1")
    run("ip", "-n", "lan", "link", "set", "lan1", "up")
    run("ip", "-n", "lan", "route", "add", "default", "via", "192.168.50.1")
    forwarding = json.loads(json.dumps(request))
    forwarding["options"]["forward_interfaces"] = "lan0"
    old_forwarding = Path("/proc/sys/net/ipv4/ip_forward").read_text()
    proc, events = worker(forwarding)
    connected(proc, events)
    assert http("10.77.0.1", namespace="lan"), "local client was not forwarded/NATed through the tunnel"
    assert not http("198.18.0.2", namespace="lan"), "local client bypassed the tunnel"
    disconnect(proc)
    stopped(proc)
    assert Path("/proc/sys/net/ipv4/ip_forward").read_text() == old_forwarding
    print("PASS local forwarding/NAT through tunnel, blocked forward bypass, sysctl restoration", flush=True)

    def set_lock(proc, events, enabled):
        proc.stdin.write("lock-enable\n" if enabled else "lock-disable\n")
        proc.stdin.flush()
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            event = events.get(timeout=5)
            if event.get("event") == "protection" and event.get("enabled") == enabled:
                return
        raise AssertionError("Network Lock toggle was not acknowledged")

    proc, events = worker(request)
    connected(proc, events)
    for _ in range(2):
        set_lock(proc, events, False)
        assert http("198.18.0.2"), "live lock disable did not restore physical traffic"
        assert http("10.77.0.1"), "live lock disable interrupted tunnel"
        set_lock(proc, events, True)
        assert not http("198.18.0.2"), "live lock enable left plaintext bypass open"
        blocked_dns()
        assert http("10.77.0.1"), "live lock enable interrupted tunnel"
    disconnect(proc)
    stopped(proc)
    assert not http("198.18.0.2"), "disconnect lost last enabled lock choice"
    proc, events = worker(request)
    connected(proc, events)
    set_lock(proc, events, False)
    disconnect(proc)
    stopped(proc)
    assert http("198.18.0.2"), "disconnect ignored explicitly disabled lock choice"
    run(*worker_command(binary, "recover"))
    assert http("198.18.0.2"), "Recover re-enabled an explicitly disabled lock"
    print("PASS live Network Lock on/off toggles preserve tunnel and final choice across disconnect", flush=True)

    failed = json.loads(json.dumps(request))
    failed["profile"]["port"] = 51831
    proc, events = worker(failed)
    stopped(proc, success=False)
    assert not http("198.18.0.2"), "handshake failure removed protection"
    blocked_dns()
    run(*worker_command(binary, "lock-disable"))
    assert http("198.18.0.2"), "Recover did not restore networking"
    print("PASS first-handshake timeout retains protection; Recover restores networking", flush=True)

    for termination in ["eof", "sigterm", "unknown-command", "switch"] + (["client-exit"] if service is not None else []):
        proc, events = worker(request)
        connected(proc, events)
        if termination == "sigterm":
            os.kill(worker_pid(proc), signal.SIGTERM)
        elif termination == "client-exit":
            proc.kill()
        elif termination in ["unknown-command", "switch"]:
            proc.stdin.write("switch\n" if termination == "switch" else "invalid\n")
            proc.stdin.flush()
        else:
            proc.stdin.close()
        stopped(proc, success=termination != "client-exit")
        assert not http("198.18.0.2"), f"{termination} released protection"
        blocked_dns()
        run(*worker_command(binary, "lock-disable"))
        assert http("198.18.0.2")
        print(f"PASS {termination} retains protection until explicit recovery", flush=True)

    proc, events = worker(request)
    connected(proc, events)
    simultaneous = subprocess.run(worker_command(binary, "transport-worker"), input=json.dumps(request)+"\n", text=True, capture_output=True, timeout=5)
    assert simultaneous.returncode != 0 and "Another networking operation" in simultaneous.stderr + simultaneous.stdout
    assert http("10.77.0.1"), "concurrent worker disturbed the live tunnel"
    disconnect(proc)
    stopped(proc)
    print("PASS concurrent worker rejected without disturbing connection", flush=True)

    expired = json.loads(json.dumps(request))
    expired["options"]["handshake_connected"] = "5"
    proc, events = worker(expired)
    connected(proc, events)
    stopped(proc, success=False)
    assert not http("198.18.0.2"), "handshake expiry released protection"
    blocked_dns()
    run(*worker_command(binary, "lock-disable"))
    print("PASS handshake expiry retains traffic/DNS protection", flush=True)

    for cycle in range(3):
        proc, events = worker(request)
        connected(proc, events)
        assert http("10.77.0.1") and http("[fd77::1]")
        disconnect(proc)
        stopped(proc)
        assert not http("198.18.0.2"), "Disconnect released protection during repeated cycles"
    run(*worker_command(binary, "lock-disable"))
    assert http("198.18.0.2")
    print("PASS three repeated dual-stack connect/disconnect cycles", flush=True)

    proc, events = worker(request)
    connected(proc, events)
    os.kill(worker_pid(proc), signal.SIGKILL)
    proc.wait(timeout=3)
    assert not http("198.18.0.2"), "worker crash removed protection"
    assert json.loads(Path("/run/wave-vpn-protection.json").read_text())["enabled"]
    run(*worker_command(binary, "lock-disable"))
    stopped(proc, success=False)
    assert http("198.18.0.2")
    assert not json.loads(Path("/run/wave-vpn-protection.json").read_text())["enabled"]
    print("PASS worker crash protection and recovery; unrelated firewall preserved", flush=True)
    import pwd
    resolve_user=pwd.getpwnam("systemd-resolve")
    # A regular-file resolver host never created resolved's runtime directory.
    Path("/run/systemd/resolve").mkdir(parents=True, exist_ok=True)
    os.chown("/run/systemd/resolve",resolve_user.pw_uid,resolve_user.pw_gid)
    os.chown(resolver_target,resolve_user.pw_uid,resolve_user.pw_gid)
    Path("/run/dbus").mkdir(exist_ok=True)
    run("dbus-daemon", "--system", "--fork", "--nopidfile")
    with open("/run/resolved-test.log", "w") as dns_log:
        dns_service = subprocess.Popen(["/usr/lib/systemd/systemd-resolved"], stdout=dns_log, stderr=dns_log)
        try:
            # Disable paging explicitly: systemd 259 may invoke less when both
            # streams are /dev/null, making a healthy client's exit be SIGPIPE.
            # Keep client errors distinct from daemon startup/exit failures.
            resolver_deadline = time.monotonic() + 45
            readiness_error = "no client result"
            while time.monotonic() < resolver_deadline:
                if dns_service.poll() is not None:
                    raise AssertionError("isolated resolved exited: " + Path("/run/resolved-test.log").read_text())
                try:
                    probe = subprocess.run(["resolvectl", "--no-pager", "status"],
                                           stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
                                           timeout=min(2, max(.1, resolver_deadline - time.monotonic())))
                    if probe.returncode == 0:
                        break
                    readiness_error = f"client exit {probe.returncode}: {probe.stderr}"
                except subprocess.TimeoutExpired:
                    readiness_error = "client timed out"
                time.sleep(.1)
            else:
                raise AssertionError("isolated resolved not ready (" + readiness_error + "): "
                                     + Path("/run/resolved-test.log").read_text())
            original_resolver = resolver.read_bytes()
            for mode in ["systemd-resolved", "resolvconf"]:
                resolved = json.loads(json.dumps(request))
                resolved["options"]["dns_mode"] = mode
                proc, events = worker(resolved)
                connected(proc, events)
                assert "10.77.0.1" in run("resolvectl", "--no-pager", "dns", "wavevpn0")
                assert "~." in run("resolvectl", "--no-pager", "domain", "wavevpn0")
                disconnect(proc)
                stopped(proc)
                print(f"PASS real isolated {mode} configuration and cleanup", flush=True)
        finally:
            if dns_service.poll() is None:
                dns_service.terminate()
                try:
                    dns_service.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    dns_service.kill()
                    dns_service.wait(timeout=3)
    config = Path("/etc/wave-vpn")
    config.mkdir(exist_ok=True)
    config.chmod(0o700)
    (config / "lock.json").write_text(json.dumps({"options": {"allow_lan": "false", "allow_ping": "false"}, "enabled": True}))
    (config / "lock.json").chmod(0o600)
    run(*worker_command(binary, "lock-restore"))
    assert not http("198.18.0.2"), "persistent lock was not restored"
    run(*worker_command(binary, "recover"))
    assert not http("198.18.0.2"), "Recover removed the persistent lock"
    lock_path = config / "lock.json"
    original_lock = lock_path.read_bytes()
    lock_path.write_text("broken")
    invalid = subprocess.run(worker_command(binary, "recover"), input="", text=True, capture_output=True, timeout=5)
    assert invalid.returncode != 0, "corrupt persistent policy accepted"
    assert not http("198.18.0.2"), "corrupt persistent policy released protection"
    lock_path.write_bytes(original_lock)
    run(*worker_command(binary, "lock-disable"))
    assert http("198.18.0.2"), "disabling persistent lock did not restore networking"
    print("PASS persistent policy restore, retention during Recover, explicit disable", flush=True)
    server.terminate()
    dns_server.terminate()


if __name__ == "__main__":
    assert os.geteuid() == 0, "run as root in the test VM"
    binary = str(Path(sys.argv[1]).resolve())
    if len(sys.argv) < 3:
        os.execvp("unshare", ["unshare", "--mount", "--net", "--pid", "--fork", "--mount-proc", sys.executable, str(Path(__file__).resolve()), binary, "--isolated"])
    main(binary)
