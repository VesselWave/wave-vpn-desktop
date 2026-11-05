#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Installs Wave VPN under /usr/local, from a source checkout (after
# `cargo build --release`) or from the extracted release tarball, where this
# script sits next to files/. Installed as /usr/local/bin/wave-vpn-uninstall,
# it removes Wave VPN again.
set -euo pipefail
self=$(readlink -f -- "${BASH_SOURCE[0]}")
here=$(dirname -- "$self")
usage() { echo 'Usage: install.sh [install|uninstall] [binary-directory]' >&2; exit 2; }
if [[ $(basename -- "$self") == wave-vpn-uninstall ]]; then
    [[ $# == 0 ]] || { echo 'Usage: sudo wave-vpn-uninstall' >&2; exit 2; }
    layout=installed action=uninstall
elif [[ -f "$here/files/usr/local/bin/wave-vpn" ]]; then
    layout=bundle action=${1:-install}
    [[ $# -le 1 ]] || usage
else
    layout=repo action=${1:-install}
    bindir=${2:-"$here/../target/release"}
fi
[[ $action == install || $action == uninstall ]] || usage
stage=${DESTDIR:-}
if [[ -n "$stage" && "$stage" != /* ]]; then echo 'DESTDIR must be absolute' >&2; exit 1; fi
if [[ -z "$stage" && $EUID != 0 ]]; then
    # Ask for the password here, so `./install.sh` is the whole instruction.
    # SUDO_UID/PKEXEC_UID tell the helper which desktop user to authorize.
    if command -v sudo > /dev/null; then exec sudo -- "$BASH" "$self" "$@"; fi
    if command -v pkexec > /dev/null; then exec pkexec "$BASH" "$self" "$@"; fi
    echo 'Run this as root.' >&2; exit 1
fi
fail() { echo "$*" >&2; exit 1; }
# Refuse symlinks in managed installation paths, and permissive live parents.
check_dir() {
    local current="$1"
    while [[ "$current" != / && -n "$current" ]]; do
        [[ ! -L "$current" ]] || fail "Refusing symlink: $current"
        if [[ -z "$stage" && -e "$current" ]]; then
            [[ $(stat -c %u "$current") == 0 ]] || fail "Not root-owned: $current"
            local mode; mode=$(stat -c %a "$current")
            (( (8#$mode & 8#022) == 0 )) || fail "Writable installation parent: $current"
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
# Wave looks for its tools in these directories, not in the caller's PATH.
have() {
    local dir
    for dir in /usr/sbin /usr/bin /sbin /bin; do [[ -x "$dir/$1" ]] && return 0; done
    return 1
}
# Name the missing tools' packages for this distribution and offer to install them.
dependencies() {
    [[ -d /run/systemd/system ]] || fail 'Wave VPN needs systemd.'
    local manager='' tool missing=() packages=()
    for manager in apt-get pacman dnf zypper ''; do [[ -z $manager ]] || ! have "$manager" || break; done
    for tool in wg ip nft secret-tool; do
        have "$tool" && continue
        missing+=("$tool")
        case "$tool:$manager" in
            wg:*) packages+=(wireguard-tools) ;;
            nft:*) packages+=(nftables) ;;
            ip:dnf) packages+=(iproute) ;;
            ip:*) packages+=(iproute2) ;;
            secret-tool:pacman | secret-tool:dnf) packages+=(libsecret) ;;
            secret-tool:zypper) packages+=(secret-tool) ;;
            secret-tool:*) packages+=(libsecret-tools) ;;
        esac
    done
    (( ${#missing[@]} )) || return 0
    local command=()
    case $manager in
        apt-get) command=(apt-get -o DPkg::Lock::Timeout=300 install -y "${packages[@]}") ;;
        pacman) command=(pacman -S --needed --noconfirm "${packages[@]}") ;;
        dnf) command=(dnf install -y "${packages[@]}") ;;
        zypper) command=(zypper --non-interactive install "${packages[@]}") ;;
    esac
    echo "Wave VPN needs: ${packages[*]}"
    if (( ${#command[@]} )) && [[ -t 0 ]]; then
        local answer=''
        read -r -p 'Install them now? [Y/n] ' answer || true
        if [[ ! $answer =~ ^[Nn] ]]; then
            if [[ $manager == apt-get ]]; then apt-get -o DPkg::Lock::Timeout=300 update -q || true; fi
            "${command[@]}" || true
        fi
    fi
    for tool in wg ip nft; do
        have "$tool" && continue
        if (( ${#command[@]} )); then fail "Install them first: sudo ${command[*]}"; fi
        fail 'Install WireGuard tools, iproute2 and nftables with your package manager first.'
    done
    have secret-tool || echo 'Without secret-tool (libsecret), Wave VPN cannot remember your account.'
}
# Disconnect, turn off Network Lock and remove the networking helper.
teardown() {
    local bin=/usr/local/bin/wave-vpn path state=''
    [[ -x $bin ]] || return 0
    # Nothing to undo unless Wave has set something up since boot or installed units.
    for path in /run/wave-vpn /run/wave-vpn-protection.json /etc/systemd/system/wave-vpn-service.socket \
        /etc/systemd/system/wave-vpn-lock.service; do
        [[ -e $path ]] && state=1
    done
    [[ -n $state ]] || return 0
    # Stopping the helper also stops a running connection.
    systemctl stop wave-vpn-service.socket wave-vpn-service.service 2> /dev/null || true
    "$bin" lock-disable < /dev/null > /dev/null
    if [[ -f /etc/systemd/system/wave-vpn-lock.service ]]; then
        "$bin" lock-uninstall < /dev/null > /dev/null
    fi
    if [[ -f /etc/systemd/system/wave-vpn-service.socket ]]; then "$bin" service-uninstall; fi
    rmdir --ignore-fail-on-non-empty /etc/wave-vpn 2> /dev/null || true
    echo 'Disconnected, turned off Network Lock and removed the networking helper.'
}
case "$action" in
install)
    if [[ $layout == repo ]]; then
        [[ -x "$bindir/wave-vpn" && -x "$bindir/wave-vpn-gui" ]] || fail 'Build both release binaries first: cargo build --release --locked --bins'
    fi
    if [[ -z "$stage" ]]; then
        for _ in {1..50}; do [[ ! -e /run/wave-vpn/recovery.json ]] && break; sleep 0.1; done
        [[ ! -e /run/wave-vpn/recovery.json ]] || fail 'Disconnect Wave VPN (or press Recover) before updating it.'
        [[ ! -e /usr/bin/wave-vpn ]] || fail 'Wave VPN is already installed from a package. Update or remove it with your package manager instead.'
        dependencies
    fi
    # Documentation is replaced as a whole, so files from older releases don't linger.
    check_dir "$stage/usr/local/share/doc/wave-vpn"
    rm -rf -- "$stage/usr/local/share/doc/wave-vpn"
    if [[ $layout == bundle ]]; then
        files="$here/files"
        [[ -z $(find "$files" ! -type f ! -type d) ]] || fail 'Unexpected links or special files in files/'
        while IFS= read -r -d '' source; do
            dest=${source#"$files"}
            [[ $dest == /usr/local/* && $dest != *..* ]] || fail "Unexpected file: files$dest"
            if [[ -x "$source" ]]; then mode=755; else mode=644; fi
            put "$source" "$dest" "$mode"
        done < <(find "$files" -type f -print0 | sort -z)
    else
        repo=$(cd -- "$here/.." && pwd)
        put "$bindir/wave-vpn" /usr/local/bin/wave-vpn 755
        put "$bindir/wave-vpn-gui" /usr/local/bin/wave-vpn-gui 755
        put "$self" /usr/local/bin/wave-vpn-uninstall 755
        put "$repo/packaging/wave-vpn.desktop" /usr/local/share/applications/wave-vpn.desktop 644
        put "$repo/resources/wave-vpn.svg" /usr/local/share/icons/hicolor/scalable/apps/wave-vpn.svg 644
        for doc in LICENSE README.md SECURITY.md; do put "$repo/$doc" "/usr/local/share/doc/wave-vpn/$doc" 644; done
        for doc in "$repo"/docs/*.md; do put "$doc" "/usr/local/share/doc/wave-vpn/docs/$(basename -- "$doc")" 644; done
        put "$repo/resources/fonts/OFL.txt" /usr/local/share/doc/wave-vpn/licenses/OFL.txt 644
        put "$repo/resources/LUCIDE-LICENSE.txt" /usr/local/share/doc/wave-vpn/licenses/LUCIDE-LICENSE.txt 644
    fi
    [[ -z "$stage" ]] || exit 0
    if [[ -f /etc/systemd/system/wave-vpn-lock.service ]]; then
        put /usr/local/bin/wave-vpn /usr/local/libexec/wave-vpn-service 755
    fi
    if [[ ${SUDO_UID:-0} != 0 || ${PKEXEC_UID:-0} != 0 || -f /etc/wave-vpn/service.json ]]; then
        /usr/local/bin/wave-vpn service-install
    else
        echo 'The networking helper will be set up the first time you press Connect.'
    fi
    cat <<'EOF'

Wave VPN is installed.
  Open it:    Wave VPN in your app menu, or run wave-vpn-gui
  Uninstall:  sudo wave-vpn-uninstall
EOF
    ;;
uninstall)
    if [[ -z "$stage" ]]; then
        teardown
    fi
    for path in /usr/local/bin /usr/local/share/applications /usr/local/share/doc/wave-vpn /usr/local/share/icons/hicolor/scalable/apps; do check_dir "$stage$path"; done
    rm -f -- "$stage/usr/local/bin/wave-vpn" "$stage/usr/local/bin/wave-vpn-gui" "$stage/usr/local/bin/wave-vpn-uninstall" \
        "$stage/usr/local/share/applications/wave-vpn.desktop" "$stage/usr/local/share/icons/hicolor/scalable/apps/wave-vpn.svg"
    rm -rf -- "$stage/usr/local/share/doc/wave-vpn"
    echo 'Removed Wave VPN. Your settings in ~/.config/wave-vpn were kept.'
    ;;
esac
