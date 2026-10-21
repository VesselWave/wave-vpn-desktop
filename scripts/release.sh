#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Assemble a candidate from a clean tagged checkout. Never signs or publishes.
set -euo pipefail
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
mkdir -p "$out"
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
name="wave-vpn-$version-$host"
DESTDIR="$work/$name" bash scripts/install.sh install
printf 'version=%s\nrevision=%s\ntarget=%s\n' "$version" "$revision" "$host" > "$work/$name/SOURCE.txt"
rustc -vV > "$work/$name/TOOLCHAIN.txt"
cargo tree --locked --prefix none > "$work/$name/DEPENDENCIES.txt"
git -C "$repo" archive --format=tar.gz --prefix="wave-vpn-$version/" HEAD > "$out/wave-vpn-$version-source.tar.gz"
# GNU tar provides stable metadata and ordering; compile reproducibility is a
# separate release gate and is not asserted by this archive operation.
SOURCE_DATE_EPOCH=$(git -C "$repo" show -s --format=%ct HEAD)
tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner -czf "$out/$name.tar.gz" -C "$work" "$name"
(cd "$out" && sha256sum "$name.tar.gz" "wave-vpn-$version-source.tar.gz" > "$name-SHA256SUMS")
echo "Candidate artifacts: $out"
echo 'Unsigned and unpublished. Verify the release gates and sign checksums with the maintainer key before distribution.'
