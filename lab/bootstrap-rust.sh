#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail
if [ ! -x "$HOME/.cargo/bin/cargo" ]; then
  installer=$(mktemp)
  trap 'rm -f "$installer"' EXIT
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o "$installer"
  sh "$installer" -y --profile minimal --component clippy
fi
"$HOME/.cargo/bin/rustup" component add clippy
"$HOME/.cargo/bin/cargo" --version
