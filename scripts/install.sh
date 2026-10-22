#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
action=${1:-install}
bindir=${2:-"$repo/target/release"}
stage=${DESTDIR:-}
if [[ -z "$stage" && $EUID != 0 ]]; then
    echo 'Run with sudo for installation, or set DESTDIR for an isolated staging test.' >&2; exit 1
fi
if [[ -n "$stage" && "$stage" != /* ]]; then echo 'DESTDIR must be absolute' >&2; exit 1; fi
# Refuse symlinks in managed installation paths, and permissive live parents.
check_dir() {
    local current="$1"
    while [[ "$current" != / && -n "$current" ]]; do
        [[ ! -L "$current" ]] || { echo "Refusing symlink: $current" >&2; exit 1; }
        if [[ -z "$stage" && -e "$current" ]]; then
            [[ $(stat -c %u "$current") == 0 ]] || { echo "Not root-owned: $current" >&2; exit 1; }
            local mode; mode=$(stat -c %a "$current")
            (( (8#$mode & 8#022) == 0 )) || { echo "Writable installation parent: $current" >&2; exit 1; }
        fi
        current=$(dirname -- "$current")
    done
}
put() {
    local source="$1" dest="$stage$2" mode="$3"
    check_dir "$(dirname -- "$dest")"
    mkdir -p -- "$(dirname -- "$dest")"
    local temporary; temporary=$(mktemp "$(dirname -- "$dest")/.wave-install.XXXXXXXX")
    install -m "$mode" -- "$source" "$temporary"
    mv -fT -- "$temporary" "$dest"
}
case "$action" in
install)
    [[ -x "$bindir/wave-vpn" && -x "$bindir/wave-vpn-gui" ]] || { echo "Build both release binaries first: cargo build --release --locked --bins" >&2; exit 1; }
    if [[ -z "$stage" ]]; then
        for _ in {1..50}; do [[ ! -e /run/wave-vpn/recovery.json ]] && break; sleep 0.1; done
        [[ ! -e /run/wave-vpn/recovery.json ]] || { echo 'Disconnect/Recover before updating Wave VPN.' >&2; exit 1; }
    fi
    put "$bindir/wave-vpn" /usr/local/bin/wave-vpn 755
    put "$bindir/wave-vpn-gui" /usr/local/bin/wave-vpn-gui 755
    put "$repo/packaging/wave-vpn.desktop" /usr/local/share/applications/wave-vpn.desktop 644
    put "$repo/resources/wave-vpn.svg" /usr/local/share/icons/hicolor/scalable/apps/wave-vpn.svg 644
    for doc in LICENSE README.md SECURITY.md; do put "$repo/$doc" "/usr/local/share/doc/wave-vpn/$doc" 644; done
    for doc in "$repo"/docs/*.md; do put "$doc" "/usr/local/share/doc/wave-vpn/$(basename -- "$doc")" 644; done
    mkdir -p "$stage/usr/local/share/doc/wave-vpn/licenses"
    put "$repo/resources/fonts/OFL.txt" /usr/local/share/doc/wave-vpn/licenses/OFL.txt 644
    put "$repo/resources/LUCIDE-LICENSE.txt" /usr/local/share/doc/wave-vpn/licenses/LUCIDE-LICENSE.txt 644
    if [[ -z "$stage" && -f /etc/systemd/system/wave-vpn-lock.service ]]; then
        put "$bindir/wave-vpn" /usr/local/libexec/wave-vpn-service 755
    fi
    if [[ -z "$stage" ]]; then /usr/local/bin/wave-vpn service-install; fi
    echo 'Installed Wave VPN. The GUI runs as your user; use wave-vpn-gui.'
    ;;
uninstall)
    if [[ -z "$stage" && ( -e /run/wave-vpn/recovery.json || -e /etc/systemd/system/wave-vpn-lock.service ) ]]; then
        echo 'Disconnect/Recover first; explicitly uninstall persistent lock with sudo wave-vpn lock-uninstall before removing binaries.' >&2; exit 1
    fi
    if [[ -z "$stage" && -x /usr/local/bin/wave-vpn ]]; then
        # Cached status can be missing/stale after a crash. Query nftables through
        # the installed backend and refuse both enabled and unknown protection.
        /usr/local/bin/wave-vpn network-status | python3 -c '
import json, sys
status = json.loads(sys.stdin.readline())["status"]
if status["enabled"] is not False:
    sys.exit("Protection is enabled or unknown. Resolve it and explicitly disable Network Lock before uninstalling.")
'
    fi
    if [[ -z "$stage" && -f /etc/systemd/system/wave-vpn-service.socket ]]; then
        /usr/local/bin/wave-vpn service-uninstall
    fi
    for path in /usr/local/bin /usr/local/share/applications /usr/local/share/doc/wave-vpn /usr/local/share/icons/hicolor/scalable/apps; do check_dir "$stage$path"; done
    rm -f -- "$stage/usr/local/bin/wave-vpn" "$stage/usr/local/bin/wave-vpn-gui" "$stage/usr/local/share/applications/wave-vpn.desktop" "$stage/usr/local/share/icons/hicolor/scalable/apps/wave-vpn.svg"
    rm -rf -- "$stage/usr/local/share/doc/wave-vpn"
    echo 'Removed installed application files. User preferences were retained.'
    ;;
*) echo 'Usage: install.sh install|uninstall [binary-directory]' >&2; exit 2;;
esac
