#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Run inside a disposable Linux VM, from any directory.
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}
cd "$repo"
cargo fmt --check
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --bins --locked
sudo python3 tests/linux_network.py "${CARGO_TARGET_DIR:-target}/debug/wave-vpn"
