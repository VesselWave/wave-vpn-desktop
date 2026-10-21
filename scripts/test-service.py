#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Run as the authorized desktop user in a disposable VM after installation.
Does not enable Network Lock or change routing; all worker inputs are invalid.
"""
import json
import os
from pathlib import Path
import socket
import subprocess
import time

assert os.environ.get('WAVE_SERVICE_TEST_VM') == '1', 'Use only a disposable Linux VM'
assert os.geteuid() != 0, 'Run as the authorized desktop user'
binary = '/usr/local/bin/wave-vpn'
def client(action, data=''):
    return subprocess.run([binary, 'service-client', action], input=data, text=True, capture_output=True, timeout=20)

assert client('recover').returncode == 0, 'Authorized unprivileged service request failed'
print('PASS desktop user can recover without sudo or a polkit prompt', flush=True)
rejected = subprocess.run(['sudo', '-n', '-u', 'nobody', binary, 'service-client', 'recover'], input='', text=True, capture_output=True, timeout=20)
assert rejected.returncode != 0, 'Foreign UID was authorized'
print('PASS unrelated local user is rejected by kernel peer credentials', flush=True)
for action in ['service-install', 'service-uninstall', '/bin/sh', 'x' * 40]:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.settimeout(5)
        connection.connect('/run/wave-vpn-service.sock')
        connection.sendall((action+'\n').encode())
        connection.shutdown(socket.SHUT_WR)
        received = b''
        while True:
            try:
                data = connection.recv(4096)
            except ConnectionResetError:
                break
            if not data:
                break
            received += data
        assert b'"success":false' in received
assert client('transport-worker', '{"profile":{}}\n').returncode != 0
print('PASS forbidden/oversized actions and malformed worker requests rejected', flush=True)
pending = subprocess.Popen([binary, 'service-client', 'transport-worker'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
try:
    pending.stdin.write('{"profile":{}}\n')
    pending.stdin.flush()
    assert pending.wait(timeout=5) != 0
finally:
    if pending.poll() is None:
        pending.kill()
        pending.wait()
    pending.stdin.close()
print('PASS worker frames and exit reach the client while controller stdin remains open', flush=True)

with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
    connection.settimeout(14)
    connection.connect('/run/wave-vpn-service.sock')
    start = time.monotonic()
    assert b'"success":false' in connection.recv(4096) or b'service' in connection.recv(4096)
    assert time.monotonic() - start < 14
print('PASS incomplete request times out without spawning a worker', flush=True)
assert client('recover').returncode == 0, 'Service did not remain usable after rejections'
metadata = Path('/usr/local/libexec/wave-vpn-helper').stat()
assert metadata.st_uid == 0 and metadata.st_mode & 0o022 == 0
mode = subprocess.run(['sudo', '-n', 'stat', '-c', '%a', '/etc/wave-vpn/service.json'], capture_output=True, text=True, check=True).stdout.strip()
assert int(mode, 8) & 0o077 == 0
print('PASS broker remains usable; helper/config ownership and permissions are restricted', flush=True)
