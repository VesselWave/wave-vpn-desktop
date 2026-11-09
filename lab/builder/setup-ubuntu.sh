#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Prepare a disposable Ubuntu LTS clone as a package builder.
# Run as root first (system packages, nFPM), then as the build user with "user" (Rust).
set -euo pipefail
# Same pins as the release workflow (.github/workflows/release-build.yml).
NFPM_VERSION=2.48.0
NFPM_SHA256=6d7f9a28b2f397934403950fcbba83f598b08d02edbe79a0815b3b4897341750
if [[ ${1:-} == user ]]; then
    if [[ ! -x $HOME/.cargo/bin/cargo ]]; then
        installer=$(mktemp)
        trap 'rm -f "$installer"' EXIT
        curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o "$installer"
        sh "$installer" -y --profile minimal --component clippy,rustfmt
    fi
    "$HOME/.cargo/bin/rustup" update stable
    "$HOME/.cargo/bin/rustc" -V
    exit 0
fi
test "$(id -u)" = 0
export DEBIAN_FRONTEND=noninteractive
# A builder needs no desktop session; keep the RAM for rustc.
systemctl set-default multi-user.target
systemctl disable --now gdm3.service 2>/dev/null || systemctl disable --now gdm.service 2>/dev/null || true
apt-get -o DPkg::Lock::Timeout=600 update
apt-get -o DPkg::Lock::Timeout=600 install -y --no-install-recommends build-essential pkg-config \
    binutils curl ca-certificates tar gzip xz-utils zstd
if ! /usr/local/bin/nfpm --version 2>/dev/null | grep -q "$NFPM_VERSION"; then
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    curl -fsSLo "$work/nfpm.tar.gz" "https://github.com/goreleaser/nfpm/releases/download/v$NFPM_VERSION/nfpm_${NFPM_VERSION}_Linux_x86_64.tar.gz"
    echo "$NFPM_SHA256  $work/nfpm.tar.gz" | sha256sum -c -
    tar -xzf "$work/nfpm.tar.gz" -C "$work" nfpm
    install -m 755 "$work/nfpm" /usr/local/bin/nfpm
fi
/usr/local/bin/nfpm --version | head -3
