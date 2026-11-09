#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Build release binaries and .deb/.rpm/Arch packages from a source tarball.
# Usage (as the build user): build.sh SOURCE.tar.gz
# Environment: SOURCE_DATE_EPOCH, WAVE_REVISION, RUN_CHECKS=0|1, CARGO_BUILD_JOBS.
set -euo pipefail
[[ $# == 1 && -f $1 ]] || { echo 'Usage: build.sh SOURCE.tar.gz' >&2; exit 2; }
: "${SOURCE_DATE_EPOCH:?}" "${WAVE_REVISION:?}"
# shellcheck disable=SC1091
. "$HOME/.cargo/env"
root=$HOME/build
src=$root/src
out=$root/out
rm -rf -- "$src" "$out"
mkdir -p "$src" "$out"
tar -xzf "$1" -C "$src"
# Persistent target dir keeps rebuilds incremental; Cargo tracks source changes.
export CARGO_TARGET_DIR=$root/target CARGO_PROFILE_DEV_DEBUG=0 SOURCE_DATE_EPOCH
cd "$src"
if [[ ${RUN_CHECKS:-0} == 1 ]]; then
    cargo fmt --check
    cargo test --locked --all-targets
    cargo clippy --locked --all-targets -- -D warnings
fi
cargo build --release --locked --bins
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
host=$(rustc -vV | sed -n 's/^host: //p')
info=$(mktemp -d)
trap 'rm -rf -- "$info"' EXIT
printf 'version=%s\nrevision=%s\ntarget=%s\n' "$version" "$WAVE_REVISION" "$host" > "$info/SOURCE.txt"
rustc -vV > "$info/TOOLCHAIN.txt"
cargo tree --locked --prefix none > "$info/DEPENDENCIES.txt"
NFPM=/usr/local/bin/nfpm bash scripts/package.sh "$out" "$CARGO_TARGET_DIR/release" "$info"
cp "$info/SOURCE.txt" "$out/SOURCE.txt"
(cd "$out" && sha256sum -- *.deb *.rpm *.pkg.tar.zst > SHA256SUMS)
ls -l "$out"
