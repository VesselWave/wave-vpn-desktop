#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Verify signing/checksum failure paths using an ephemeral synthetic signing key."""
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
repo = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='wave-release-verification-') as directory:
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
    manifest = root / 'SHA256SUMS'
    manifest.write_text(hashlib.sha256(archive.read_bytes()).hexdigest() + '  ' + archive.name + '\n')
    signature = root / 'SHA256SUMS.asc'
    run('gpg','--batch','--armor','--detach-sign','--output',str(signature),str(manifest))
    verify = ['bash',str(repo / 'scripts/verify-release.sh'),str(public),str(manifest),str(signature)]
    run(*verify, fingerprint)
    run(*verify, '0' * len(fingerprint), ok=False)
    archive.write_bytes(b'tampered content')
    run(*verify, fingerprint, ok=False)
    manifest.write_text('0' * 64 + '  wave-test.tar.gz\n')
    run(*verify, fingerprint, ok=False)
    run('gpgconf','--kill','gpg-agent')
print('PASS release signature/fingerprint/checksum verification; tampering and wrong trust rejected')
