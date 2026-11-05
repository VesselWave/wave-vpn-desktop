#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Verify in an isolated keyring against a separately trusted full fingerprint.
set -euo pipefail
[[ $# == 4 ]] || { echo 'Usage: verify-release.sh PUBLIC_KEY SHA256SUMS SIGNATURE TRUSTED_FULL_FINGERPRINT' >&2; exit 2; }
public_key=$1
sums=$2
signature=$3
fingerprint=$(printf '%s' "$4" | tr -d ' ' | tr '[:lower:]' '[:upper:]')
[[ "$fingerprint" =~ ^[A-F0-9]{40}$ || "$fingerprint" =~ ^[A-F0-9]{64}$ ]] || { echo 'A full trusted fingerprint is required' >&2; exit 1; }
# Keep GnuPG socket paths below macOS's Unix-domain socket length limit.
keyhome=$(mktemp -d /tmp/wave-verify.XXXXXXXX)
trap 'rm -rf -- "$keyhome"' EXIT
chmod 700 "$keyhome"
gpg --homedir "$keyhome" --batch --quiet --import "$public_key"
actual=$(gpg --homedir "$keyhome" --batch --with-colons --list-keys --fingerprint | awk -F: '$1=="pub"{pub++;want=1} $1=="fpr" && want{print $10;want=0} END{if(pub!=1)exit 1}')
[[ "$actual" == "$fingerprint" ]] || { echo 'Public key does not match the trusted fingerprint' >&2; exit 1; }
gpg --homedir "$keyhome" --batch --verify "$signature" "$sums"
python3 - "$sums" <<'PY'
import hashlib
from pathlib import Path
import re
import sys
manifest = Path(sys.argv[1]).resolve()
seen = set()
for line in manifest.read_text().splitlines():
    # Archives and the .deb, .rpm and Arch packages; never paths or other file types.
    match = re.fullmatch(r'([a-fA-F0-9]{64}) [ *]([A-Za-z0-9][A-Za-z0-9._-]*\.(?:tar\.gz|deb|rpm|pkg\.tar\.zst))', line)
    if not match or match[2] in seen:
        raise SystemExit('Invalid or duplicate checksum entry')
    seen.add(match[2])
    path = manifest.parent / match[2]
    if path.is_symlink() or not path.is_file():
        raise SystemExit('Missing or unsafe release artifact')
    digest = hashlib.sha256()
    with path.open('rb') as artifact:
        for chunk in iter(lambda: artifact.read(1024*1024), b''):
            digest.update(chunk)
    if digest.hexdigest() != match[1].lower():
        raise SystemExit('Release checksum mismatch')
if not seen:
    raise SystemExit('Empty checksum manifest')
print('PASS signature, trusted fingerprint and all artifact checksums')
PY
