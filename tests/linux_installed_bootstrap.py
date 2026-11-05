#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Installed-helper upgrade/reboot gate in a dedicated disposable VM.

Run as the authorized desktop user, after installing the current binaries.
prepare OLD_BINARY STATE_FILE leaves a persistent lock enabled for a reboot.
finish STATE_FILE checks a changed boot, then removes the test lock. The caller
must retain out-of-band guest-agent recovery and uninstall the test application.
Refuses a pre-existing persistent lock. Uses only synthetic bootstrap pairs.
Keep both this source tree and STATE_FILE on reboot-persistent storage: some
guests clear /tmp at boot. STATE_FILE is written with mode 0600.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


def run(*args, data="", check=True):
    return subprocess.run(args, input=data, text=True, capture_output=True, check=check, timeout=40)


def root(*args, **kwargs):
    return run("sudo", "-n", *args, **kwargs)


BINARY = "/usr/local/bin/wave-vpn"
LOCK = "/etc/wave-vpn/lock.json"
PAIRS = [{"ip": "9.9.9.9", "port": 8443}, {"ip": "2606:4700::1111", "port": 9443}]
INPUT = json.dumps({"options": {"allow_lan": "true", "allow_ping": "false"}, "bootstrap": PAIRS})


def client(action, data=INPUT, check=True):
    return run(BINARY, "service-client", action, data=data, check=check)


def verify_rules():
    rules = root("nft", "-n", "list", "table", "inet", "wave_vpn").stdout
    for e in PAIRS:
        family = "ip6" if ":" in e["ip"] else "ip"
        assert (f'meta skuid {os.getuid()} {family} daddr {e["ip"]} '
                f'tcp dport {e["port"]} accept') in rules
    assert "policy drop" in rules


def prepare(old, state):
    assert root("test", "!", "-e", LOCK, check=False).returncode == 0, "pre-existing lock"
    root(BINARY, "lock-install", data=INPUT)
    client("lock-enable")
    client("lock-update")
    verify_rules()
    before = root("cat", LOCK).stdout
    # An actual historical, natively built helper handles the new client's
    # wire frames. The GUI uses this same service-client path and LockInput.
    trusted_old = "/usr/local/libexec/wave-vpn-old-acceptance"
    root("install", "-m", "755", old, trusted_old)
    try:
        root(trusted_old, "service-install", "--uid", str(os.getuid()))
    finally:
        root("rm", "-f", trusted_old)
    for action in ["lock-update", "lock-enable"]:
        rejected = client(action, check=False)
        assert rejected.returncode != 0, "old helper accepted new wire input"
    assert root("cat", LOCK).stdout == before
    verify_rules()
    print("PASS real old helper rejects new lock action/input without changing active protection", flush=True)
    repo = Path(__file__).resolve().parents[1]
    root("env", f"SUDO_UID={os.getuid()}", "bash", str(repo / "scripts/install.sh"), "install")
    assert root("cat", LOCK).stdout == before, "installer lost persistent lock"
    expected = hashlib.sha256(Path(repo / "target/release/wave-vpn").read_bytes()).digest()
    for p in ["/usr/local/libexec/wave-vpn-helper", "/usr/local/libexec/wave-vpn-service"]:
        # Compare binary hashes without decoding executable bytes as text.
        assert root("sha256sum", p).stdout.split()[0] == expected.hex()
    client("lock-update")
    client("recover", "")
    verify_rules()
    assert json.loads(root("cat", LOCK).stdout)["bootstrap"] == PAIRS
    print("PASS installer upgrades broker and boot executable, preserves lock and permits update/Recover", flush=True)
    state.write_text(json.dumps({"boot": Path("/proc/sys/kernel/random/boot_id").read_text(),
                                 "lock": root("cat", LOCK).stdout}))
    state.chmod(0o600)


def finish(state):
    saved = json.loads(state.read_text())
    assert Path("/proc/sys/kernel/random/boot_id").read_text() != saved["boot"], "no actual reboot"
    assert root("systemctl", "is-active", "wave-vpn-lock.service").stdout.strip() == "active"
    assert root("cat", LOCK).stdout == saved["lock"]
    verify_rules()
    client("recover", "")
    verify_rules()
    print("PASS actual reboot restores persistent IPv4/IPv6 scoped pairs; Recover retains protection", flush=True)
    client("lock-disable")
    root(BINARY, "lock-uninstall", data=INPUT)
    assert root("test", "!", "-e", LOCK, check=False).returncode == 0
    assert root("nft", "list", "table", "inet", "wave_vpn", check=False).returncode != 0
    print("PASS explicit disable/uninstall restores disconnected lock-off baseline", flush=True)


if __name__ == "__main__":
    assert os.environ.get("WAVE_SERVICE_TEST_VM") == "1" and os.geteuid() != 0
    if sys.argv[1] == "prepare":
        prepare(str(Path(sys.argv[2]).resolve()), Path(sys.argv[3]))
    elif sys.argv[1] == "finish":
        finish(Path(sys.argv[2]))
    else:
        raise SystemExit("prepare OLD_BINARY STATE_FILE | finish STATE_FILE")
