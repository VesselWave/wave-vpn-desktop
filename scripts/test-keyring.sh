#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo"
if [[ ${WAVE_ISOLATED_KEYRING_TEST:-} != 1 ]]; then
    temporary=$(mktemp -d)
    trap 'rm -rf -- "$temporary"' EXIT
    mkdir -p "$temporary/data" "$temporary/config" "$temporary/run"
    chmod 700 "$temporary/run"
    dbus-run-session -- env WAVE_ISOLATED_KEYRING_TEST=1 XDG_DATA_HOME="$temporary/data" XDG_CONFIG_HOME="$temporary/config" XDG_RUNTIME_DIR="$temporary/run" bash "$0"
    exit
fi
# Only a new synthetic collection on a private bus; never unlock the user's keyring.
printf '%s' synthetic-collection-password | gnome-keyring-daemon --unlock --components=secrets
cargo test --locked --test credentials -- --ignored --skip imports_profile_written_by_installed_eddie
