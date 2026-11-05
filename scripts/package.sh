#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Build the .deb, .rpm and Arch packages from release binaries with nFPM
# (https://nfpm.goreleaser.com; set NFPM to its path if it isn't on PATH).
# Usage: package.sh OUTPUT_DIR [BINARY_DIR] [BUILD_INFO_DIR]
set -euo pipefail
[[ $# -ge 1 && $# -le 3 ]] || { echo 'Usage: package.sh OUTPUT_DIR [BINARY_DIR] [BUILD_INFO_DIR]' >&2; exit 2; }
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
out=$(mkdir -p -- "$1" && cd -- "$1" && pwd)
bindir=$(cd -- "${2:-$repo/target/release}" && pwd)
info=${3:+$(cd -- "$3" && pwd)}
nfpm=${NFPM:-nfpm}
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo/Cargo.toml" | head -1)
[[ -x "$bindir/wave-vpn" && -x "$bindir/wave-vpn-gui" ]] || { echo "Missing binaries in $bindir" >&2; exit 1; }
[[ $(uname -m) == x86_64 ]] || { echo 'Packages are built for x86_64 only' >&2; exit 1; }
# Require the newest glibc symbol version the binaries actually use.
glibc=$(objdump -T "$bindir/wave-vpn" "$bindir/wave-vpn-gui" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -1)
[[ -n $glibc ]] || { echo 'Could not read the glibc requirement' >&2; exit 1; }
# Stable timestamps: the commit time unless the caller already set one.
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git -C "$repo" show -s --format=%ct HEAD)}
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
doc=/usr/share/doc/wave-vpn
{
    cat <<EOF
name: wave-vpn
arch: amd64
platform: linux
version: $version
release: 1
section: net
priority: optional
maintainer: VesselWave <vesselwave@protonmail.com>
vendor: VesselWave
homepage: https://vpn.vwave.me
license: GPL-3.0-or-later
description: Unofficial open-source WireGuard client for AirVPN
contents:
  - {src: '$bindir/wave-vpn', dst: /usr/bin/wave-vpn, file_info: {mode: 0755}}
  - {src: '$bindir/wave-vpn-gui', dst: /usr/bin/wave-vpn-gui, file_info: {mode: 0755}}
  - {src: '$repo/packaging/wave-vpn.desktop', dst: /usr/share/applications/wave-vpn.desktop, file_info: {mode: 0644}}
  - {src: '$repo/resources/wave-vpn.svg', dst: /usr/share/icons/hicolor/scalable/apps/wave-vpn.svg, file_info: {mode: 0644}}
  - {src: '$repo/resources/fonts/OFL.txt', dst: $doc/licenses/OFL.txt, file_info: {mode: 0644}}
  - {src: '$repo/resources/LUCIDE-LICENSE.txt', dst: $doc/licenses/LUCIDE-LICENSE.txt, file_info: {mode: 0644}}
EOF
    for file in "$repo"/LICENSE "$repo"/README.md "$repo"/SECURITY.md ${info:+"$info"/*.txt}; do
        echo "  - {src: '$file', dst: '$doc/$(basename -- "$file")', file_info: {mode: 0644}}"
    done
    for file in "$repo"/docs/*.md; do
        echo "  - {src: '$file', dst: '$doc/docs/$(basename -- "$file")', file_info: {mode: 0644}}"
    done
    cat <<EOF
scripts:
  postinstall: $repo/packaging/scripts/postinstall.sh
  preremove: $repo/packaging/scripts/preremove.sh
overrides:
  deb:
    depends: [wireguard-tools, iproute2, nftables, libsecret-tools, 'pkexec | policykit-1', systemd, 'libc6 (>= $glibc)', libgcc-s1]
  rpm:
    depends: [wireguard-tools, '(iproute or iproute2)', nftables, /usr/bin/secret-tool, polkit, systemd, 'glibc >= $glibc']
  archlinux:
    depends: [wireguard-tools, iproute2, nftables, libsecret, polkit, systemd, 'glibc>=$glibc', gcc-libs]
rpm:
  summary: Unofficial open-source WireGuard client for AirVPN
archlinux:
  packager: VesselWave <vesselwave@protonmail.com>
  scripts:
    postupgrade: $repo/packaging/scripts/postinstall.sh
EOF
} > "$work/nfpm.yaml"
for packager in deb rpm archlinux; do
    "$nfpm" package --config "$work/nfpm.yaml" --packager "$packager" --target "$out/"
done
