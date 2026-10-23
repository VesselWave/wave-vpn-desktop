#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Synthetic n/p/s profile round trips through installed Eddie on a private keyring.
No real account, tunnel, system DNS change, or normal desktop keyring access.
"""
import hashlib
import os
from pathlib import Path
import secrets
import signal
import subprocess
import sys
import tempfile
import time

assert os.environ.get('WAVE_EDDIE_DESKTOP_TEST') == '1', 'Run in a disposable Linux desktop VM'
if os.environ.get('WAVE_EDDIE_PRIVATE_BUS') != '1':
    with tempfile.TemporaryDirectory(prefix='wave-eddie-private-bus-') as directory:
        env = dict(os.environ, WAVE_EDDIE_PRIVATE_BUS='1')
        for key, folder in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_RUNTIME_DIR','run')]:
            path = Path(directory) / folder
            path.mkdir(mode=0o700)
            env[key] = str(path)
        for key in ['GNOME_KEYRING_CONTROL','SSH_AUTH_SOCK']:
            env.pop(key, None)
        subprocess.run(['dbus-run-session',sys.executable,__file__], env=env, check=True)
    sys.exit(0)
subprocess.run(['gnome-keyring-daemon','--unlock','--components=secrets'], input=b'synthetic-eddie-test-collection', stdout=subprocess.DEVNULL, check=True)
repo = Path(__file__).resolve().parents[1]
cli = Path('/usr/lib/eddie-ui/eddie-cli')
assert cli.is_file(), 'Install the official Eddie package first'
version = subprocess.run(['dpkg-query','-W','-f=${Version}','eddie-ui'], capture_output=True, text=True, check=True).stdout.strip()
print('Testing installed official Eddie ' + version, flush=True)
for variant in ['n','p','s']:
    identifier = secrets.token_hex(32)
    with tempfile.TemporaryDirectory(prefix='wave-eddie-interop-') as directory:
        base = Path(directory) / 'synthetic'
        profile = base.with_suffix('.profile')
        seed = bytearray((repo / ('protocol-data/fixtures/eddie-import/v2' + ('n' if variant == 'n' else 'p') + '-sha256.bin')).read_bytes())
        seed[:3] = ('v2' + variant).encode()
        seed[3:67] = identifier.encode()
        profile.write_bytes(seed)
        profile.chmod(0o600)
        attrs = ['Eddie Profile',identifier]
        if variant == 's':
            subprocess.run(['secret-tool','store','--label=Wave synthetic Eddie interoperability',*attrs], input=b'profile-test-password\n', check=True, timeout=60)
        try:
            with (Path(directory) / 'eddie.log').open('wb') as log:
                # Background lab launchers can inherit SIGINT=SIG_IGN. Reset it
                # for Eddie so the clean-save request is actually delivered.
                child = subprocess.Popen(['/usr/bin/env','--default-signal=INT',str(cli),'--path.resources=/usr/share/eddie-ui','--profile=' + str(base),'--remember=true','--connect=false','--netlock=false','--dns.mode=none'], stdin=subprocess.PIPE, stdout=log, stderr=log)
                try:
                    if variant == 'p':
                        child.stdin.write(b'profile-test-password\n')
                        child.stdin.flush()
                    time.sleep(4)
                    child.send_signal(signal.SIGINT)
                    child.wait(timeout=45)
                finally:
                    child.stdin.close()
                    if child.poll() is None:
                        child.kill()
                        child.wait()
            assert child.returncode == 0, 'Installed Eddie did not exit cleanly'
            written = profile.read_bytes()
            assert written[:3] == ('v2' + variant).encode(), 'Eddie changed profile protection unexpectedly'
            assert hashlib.sha256(written).digest() != hashlib.sha256(seed).digest(), 'Eddie did not save the profile'
            env = dict(os.environ, WAVE_EDDIE_TEST_PROFILE=str(profile), WAVE_EDDIE_TEST_PASSWORD='profile-test-password' if variant == 'p' else '')
            subprocess.run(['cargo','test','--manifest-path',str(repo / 'Cargo.toml'),'--locked','--test','credentials','imports_profile_written_by_installed_eddie','--','--ignored','--exact'], env=env, check=True, timeout=180)
            assert profile.read_bytes() == written, 'Wave modified the Eddie profile'
            if variant == 's':
                assert subprocess.run(['secret-tool','lookup',*attrs], stdout=subprocess.DEVNULL, timeout=15).returncode == 0, 'Wave removed the Eddie item'
            print('PASS official Eddie ' + version + ' writes v2' + variant + '; Wave imports without modifying source', flush=True)
        finally:
            if variant == 's':
                subprocess.run(['secret-tool','clear',*attrs], check=True, timeout=15)
