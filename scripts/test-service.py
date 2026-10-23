#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Run as the authorized desktop user in a disposable VM after installation.
Does not enable Network Lock or change routing; all worker inputs are invalid.
Temporarily changes the broker authorization UID, restoring it in finally.
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

# Fill all eight authorized slots with incomplete requests. An unauthorized
# peer must still get its UID rejection, rather than the saturation response.
# This deterministically fails a broker that reserves slots before peer auth.
held = []
try:
    for _ in range(8):
        connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        connection.settimeout(5)
        connection.connect('/run/wave-vpn-service.sock')
        held.append(connection)
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as probe:
        probe.settimeout(5)
        probe.connect('/run/wave-vpn-service.sock')
        response = probe.makefile('rb').read()
        assert b'"success":false' in response, response
    burst = subprocess.Popen([
        'sudo', '-n', '-u', 'nobody', 'python3', '-u', '-c', r'''import concurrent.futures, socket, sys, threading, time
def connect(_):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
        s.settimeout(5)
        s.connect('/run/wave-vpn-service.sock')
        with s.makefile('rb') as reader:
            response = reader.read()
        assert b'This user is not authorized' in response, response
        assert b'"success":false' in response, response
with concurrent.futures.ThreadPoolExecutor(max_workers=16) as pool:
    list(pool.map(connect, range(32)))
print('REJECTED 32', flush=True)
assert sys.stdin.readline().strip() == 'progress'
stop = threading.Event()
started = threading.Barrier(5)
counts = [0] * 4
def produce(index):
    connect(index)
    counts[index] += 1
    started.wait(timeout=8)
    deadline = time.monotonic() + 45
    while not stop.is_set():
        assert time.monotonic() < deadline, 'Authorized progress exceeded burst deadline'
        connect(index)
        counts[index] += 1
        # Bound pressure on the shared VM while keeping producers overlapping.
        time.sleep(0.01)
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    jobs = [pool.submit(produce, i) for i in range(4)]
    started.wait(timeout=8)
    print('RUNNING', flush=True)
    assert sys.stdin.readline().strip() == 'release'
    stop.set()
    for job in jobs:
        job.result(timeout=8)
assert min(counts) > 1, counts
print('PROGRESS_REJECTIONS', sum(counts), flush=True)
'''
    ], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        # select bounds the readiness wait even if the child hangs or crashes.
        import select
        assert select.select([burst.stdout], [], [], 8)[0], 'Foreign burst timed out'
        assert burst.stdout.readline().strip() == 'REJECTED 32', 'Foreign rejection assertions failed'
        for connection in held:
            connection.close()
        held.clear()
        burst.stdin.write('progress\n')
        burst.stdin.flush()
        assert select.select([burst.stdout], [], [], 8)[0], 'Foreign producers did not start'
        assert burst.stdout.readline().strip() == 'RUNNING', 'Foreign producer startup failed'
        # Authorized requests now run while foreign producers are active.
        deadline = time.monotonic() + 5
        while client('recover').returncode != 0:
            assert time.monotonic() < deadline, 'Authorized client could not recover'
            time.sleep(0.05)
        for _ in range(8):
            assert client('network-status').returncode == 0, 'Authorized progress failed'
        burst.stdin.write('release\n')
        burst.stdin.flush()
        output, errors = burst.communicate(timeout=10)
        assert burst.returncode == 0, errors
        assert 'PROGRESS_REJECTIONS ' in output, output
    finally:
        if burst.poll() is None:
            burst.kill()
            burst.communicate()
finally:
    for connection in held:
        connection.close()
print('PASS 32 foreign peers rejected with all slots occupied; authorized progress during sustained foreign traffic', flush=True)

# A root-managed authorization update takes effect before slot reservation.
# Restore the exact config even if the assertion or socket operation fails.
config_path = '/etc/wave-vpn/service.json'
original_config = subprocess.run(['sudo', '-n', 'cat', config_path], capture_output=True, check=True).stdout
try:
    import pwd
    replacement = json.dumps({'uid': pwd.getpwnam('nobody').pw_uid}).encode()
    subprocess.run(['sudo', '-n', 'tee', config_path], input=replacement, stdout=subprocess.DEVNULL, check=True)
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.settimeout(5)
        connection.connect('/run/wave-vpn-service.sock')
        response = connection.makefile('rb').read()
        assert b'This user is not authorized' in response, response
        assert b'"success":false' in response, response
finally:
    subprocess.run(['sudo', '-n', 'tee', config_path], input=original_config, stdout=subprocess.DEVNULL, check=True)
assert client('recover').returncode == 0, 'Restored authorization did not take effect'
print('PASS revoked UID rejected at admission; restored UID works without broker restart', flush=True)

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
