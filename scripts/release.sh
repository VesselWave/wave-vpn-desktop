#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Assemble a candidate from a clean tagged checkout. Optional local signing; never publishes.
set -euo pipefail
signing_key=""
if [[ ${1:-} == --sign-key && $# == 2 ]]; then
    signing_key=$2
elif [[ $# != 0 ]]; then
    echo 'Usage: release.sh [--sign-key FULL_FINGERPRINT]' >&2; exit 2
fi
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo"
[[ -z $(git -C "$repo" status --porcelain) ]] || { echo 'Release requires a clean checkout' >&2; exit 1; }
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
revision=$(git -C "$repo" rev-parse HEAD)
[[ $(git -C "$repo" rev-parse "v$version^{commit}") == "$revision" ]] || { echo "Tag v$version must identify HEAD" >&2; exit 1; }
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked --bins
host=$(rustc -vV | sed -n 's/^host: //p')
[[ "$host" == *-linux-* ]] || { echo 'Linux release artifacts must be built on Linux' >&2; exit 1; }
out="$repo/target/distribution"
rm -rf -- "$out"
mkdir -p "$out"
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
# The tarball: README.txt, install.sh and LICENSE on top, the files to install below.
name="wave-vpn-$version-$host"
bundle="$work/$name"
DESTDIR="$bundle/files" bash scripts/install.sh install
info="$bundle/files/usr/local/share/doc/wave-vpn"
printf 'version=%s\nrevision=%s\ntarget=%s\n' "$version" "$revision" "$host" > "$info/SOURCE.txt"
rustc -vV > "$info/TOOLCHAIN.txt"
cargo tree --locked --prefix none > "$info/DEPENDENCIES.txt"
install -m 755 scripts/install.sh "$bundle/install.sh"
install -m 644 LICENSE "$bundle/LICENSE"
sed "s/@VERSION@/$version/g" packaging/README.txt > "$bundle/README.txt"
chmod 644 "$bundle/README.txt"
git -C "$repo" archive --format=tar.gz --prefix="wave-vpn-$version/" HEAD > "$out/wave-vpn-$version-source.tar.gz"
# GNU tar provides stable metadata and ordering; compile reproducibility is a
# separate release gate and is not asserted by this archive operation.
SOURCE_DATE_EPOCH=$(git -C "$repo" show -s --format=%ct HEAD)
export SOURCE_DATE_EPOCH
tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner -czf "$out/$name.tar.gz" -C "$work" "$name"
bash scripts/package.sh "$out" "$repo/target/release" "$info"
packages=("wave-vpn_$version-1_amd64.deb" "wave-vpn-$version-1.x86_64.rpm" "wave-vpn-$version-1-x86_64.pkg.tar.zst")
(cd "$out" && sha256sum "${packages[@]}" "$name.tar.gz" "wave-vpn-$version-source.tar.gz" > "$name-SHA256SUMS")
echo "Candidate artifacts: $out"
if [[ -n "$signing_key" ]]; then
    [[ "$signing_key" =~ ^[A-Fa-f0-9]{40}$ || "$signing_key" =~ ^[A-Fa-f0-9]{64}$ ]] || { echo 'Use a full signing-key fingerprint' >&2; exit 1; }
    gpg --local-user "$signing_key" --armor --detach-sign --output "$out/$name-SHA256SUMS.asc" "$out/$name-SHA256SUMS"
    gpg --armor --export "$signing_key" > "$out/$name-signing-key.asc"
    bash scripts/verify-release.sh "$out/$name-signing-key.asc" "$out/$name-SHA256SUMS" "$out/$name-SHA256SUMS.asc" "$signing_key"
    echo 'Signed candidate, not published. Confirm docs/RELEASE-GATES.md before distribution.'
else
    echo 'Unsigned candidate, not published. Run with --sign-key FULL_FINGERPRINT to sign locally.'
fi
