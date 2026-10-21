#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Run only in a disposable Linux desktop VM with an unlocked Secret Service.
Uses installed official Eddie to rewrite a synthetic keyring profile, then Wave
imports it. Does not connect a tunnel or use a real AirVPN account.
"""
import hashlib
import os
from pathlib import Path
import secrets
import signal
import subprocess
import tempfile
import time

assert os.environ.get("WAVE_EDDIE_DESKTOP_TEST") == "1", "Use an unlocked disposable desktop VM"
repo = Path(__file__).resolve().parents[1]
cli = Path("/usr/lib/eddie-ui/eddie-cli")
assert cli.is_file(), "Install official Eddie UI package first"
identifier = secrets.token_hex(32)
with tempfile.TemporaryDirectory(prefix="wave-eddie-interop-") as directory:
    base = Path(directory) / "synthetic"
    profile = base.with_suffix(".profile")
    seed = bytearray((repo / "protocol-data/fixtures/eddie-import/v2p-sha256.bin").read_bytes())
    seed[:3] = b"v2s"
    seed[3:67] = identifier.encode()
    profile.write_bytes(seed)
    profile.chmod(0o600)
    attrs = ["Eddie Profile", identifier]
    subprocess.run(["secret-tool", "store", "--label=Wave synthetic Eddie interoperability", *attrs], input=b"profile-test-password\n", check=True, timeout=60)
    try:
        with (Path(directory) / "eddie.log").open("wb") as log:
            child = subprocess.Popen([str(cli), "--path.resources=/usr/share/eddie-ui", "--profile=" + str(base), "--remember=true", "--connect=false", "--netlock=false", "--dns.mode=none"], stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            try:
                time.sleep(4)
                child.send_signal(signal.SIGINT)
                child.wait(timeout=45)
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
        assert child.returncode == 0, "Installed Eddie did not exit cleanly; inspect its runtime before testing import"
        written = profile.read_bytes()
        assert written[:3] == b"v2s", "Eddie could not use the desktop keyring"
        assert hashlib.sha256(written).digest() != hashlib.sha256(seed).digest(), "Eddie did not save the profile"
        env = dict(os.environ, WAVE_EDDIE_TEST_PROFILE=str(profile))
        subprocess.run(["cargo", "test", "--manifest-path", str(repo / "Cargo.toml"), "--locked", "--test", "credentials", "imports_profile_written_by_installed_eddie", "--", "--ignored", "--exact"], env=env, check=True, timeout=180)
        assert profile.read_bytes() == written, "Wave modified Eddie's profile"
        assert subprocess.run(["secret-tool", "lookup", *attrs], stdout=subprocess.DEVNULL, timeout=15).returncode == 0, "Wave removed Eddie's keyring item"
        print("PASS installed Eddie rewrites v2s; Wave imports credentials using GNOME keyring, leaving profile and item intact", flush=True)
    finally:
        subprocess.run(["secret-tool", "clear", *attrs], check=True, timeout=15)
