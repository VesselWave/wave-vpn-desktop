#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Authenticate privately on the host, then test isolated VPN traffic in a Lima VM.
The .env format is KEY=<JSON string>, as written by the private prompt command.
Only worker profiles cross to the guest, through stdin. Temporary profiles are
owner-only and automatically deleted. Raw private diagnostics are not printed.
"""
import argparse
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--vm', required=True, help='Disposable Lima Linux VM')
    parser.add_argument('--guest-binary', required=True, help='Absolute Linux wave-vpn binary path')
    parser.add_argument('--guest-repo', help='Repository path mounted in the guest (defaults to host path)')
    parser.add_argument('--env-file', type=Path, help='Private credentials file (default: repository .env)')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    path = args.env_file or repo / '.env'
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd) as file:
        metadata = os.fstat(file.fileno())
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_mode & 0o077 or metadata.st_uid != os.getuid():
            raise RuntimeError('Credentials file must be a private regular file owned by this user')
        data = file.read(16385)
    if len(data) > 16384:
        raise RuntimeError('Credentials file too large')
    values = {}
    for line in data.splitlines():
        key, separator, value = line.partition('=')
        if separator and key in ('AIRVPN_USERNAME', 'AIRVPN_PASSWORD'):
            if key in values:
                raise RuntimeError('Duplicate credential field')
            values[key] = json.loads(value)
    if not all(isinstance(values.get(key), str) and values[key] for key in ('AIRVPN_USERNAME', 'AIRVPN_PASSWORD')):
        raise RuntimeError('Missing account credentials')
    credential_json = json.dumps({'login': values['AIRVPN_USERNAME'], 'password': values['AIRVPN_PASSWORD']}).encode()
    with tempfile.TemporaryDirectory(prefix='wave-private-live-') as directory:
        requests = Path(directory) / 'requests.json'
        env = dict(os.environ, WAVE_AUTH_OUTPUT=str(requests))
        subprocess.run(['cargo', 'test', '--manifest-path', str(repo / 'Cargo.toml'), '--locked', '--test', 'authenticated', '--', '--ignored', '--nocapture'], input=credential_json, env=env, check=True, timeout=180)
        guest_repo = args.guest_repo or str(repo)
        with requests.open('rb') as private:
            subprocess.run(['limactl', 'shell', args.vm, '--', 'sudo', 'env', 'WAVE_LIVE_TEST_VM=1', 'python3', guest_repo + '/tests/live_linux.py', args.guest_binary], stdin=private, check=True, timeout=510)


if __name__ == '__main__':
    main()
