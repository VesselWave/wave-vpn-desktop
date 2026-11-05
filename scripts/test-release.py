#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Verify signing/checksum failure paths using an ephemeral synthetic signing key."""
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
repo = Path(__file__).resolve().parents[1]
# macOS TMPDIR can exceed the Unix-domain socket limit once GnuPG appends
# its agent socket name. mkdtemp still creates this short directory privately.
with tempfile.TemporaryDirectory(prefix='wave-release-', dir='/tmp') as directory:
    root = Path(directory)
    keyhome = root / 'keyring'
    keyhome.mkdir(mode=0o700)
    env = dict(os.environ, GNUPGHOME=str(keyhome))
    def run(*args, ok=True, **kwargs):
        result = subprocess.run(args, env=env, capture_output=True, **kwargs)
        assert (result.returncode == 0) == ok, 'Unexpected release verification result: ' + result.stderr.decode(errors='replace')[:2000]
        return result
    run('gpg','--batch','--pinentry-mode','loopback','--passphrase','','--quick-generate-key','Wave synthetic release test','ed25519','sign','0')
    listing = run('gpg','--batch','--with-colons','--list-keys').stdout.decode()
    fingerprint = next(line.split(':')[9] for line in listing.splitlines() if line.startswith('fpr:'))
    public = root / 'public.asc'
    public.write_bytes(run('gpg','--armor','--export',fingerprint).stdout)
    archive = root / 'wave-test.tar.gz'
    archive.write_bytes(b'synthetic archive content')
    packages = [root / 'wave-test_1.0-1_amd64.deb', root / 'wave-test-1.0-1.x86_64.rpm', root / 'wave-test-1.0-1-x86_64.pkg.tar.zst']
    for package in packages:
        package.write_bytes(b'synthetic package ' + package.name.encode())
    manifest = root / 'SHA256SUMS'
    signature = root / 'SHA256SUMS.asc'
    def sign(files):
        manifest.write_text(''.join(hashlib.sha256(f.read_bytes()).hexdigest() + '  ' + f.name + '\n' for f in files))
        signature.unlink(missing_ok=True)
        run('gpg','--batch','--armor','--detach-sign','--output',str(signature),str(manifest))
    verify = ['bash',str(repo / 'scripts/verify-release.sh'),str(public),str(manifest),str(signature)]
    sign([*packages, archive])
    run(*verify, fingerprint)
    run(*verify, '0' * len(fingerprint), ok=False)
    # A signed manifest still may not name other file types.
    script = root / 'wave-test.sh'
    script.write_bytes(b'#!/bin/sh\n')
    sign([archive, script])
    run(*verify, fingerprint, ok=False)
    sign([archive])
    archive.write_bytes(b'tampered content')
    run(*verify, fingerprint, ok=False)
    manifest.write_text('0' * 64 + '  wave-test.tar.gz\n')
    run(*verify, fingerprint, ok=False)
    run('gpgconf','--kill','gpg-agent')
print('PASS release signature/fingerprint/checksum verification for archives and packages; tampering, wrong trust and other file types rejected')
