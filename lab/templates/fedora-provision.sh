#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Turn a Fedora Cloud Base image into a Fedora Workstation-equivalent test template.
# Run as root inside a dedicated disposable guest: fedora-provision.sh install|seal
# Environment: WAVE_LAB_USER (default wave-lab). WAVE_APT_PROXY is ignored here.
# The login password is read once from /root/.wave-lab-password and deleted.
set -euo pipefail
user=${WAVE_LAB_USER:-wave-lab}
phase=${1:?usage: fedora-provision.sh install|seal}

install_phase() {
    cloud-init status --wait >/dev/null 2>&1 || true
    # Deviation: no background package activity during tests.
    systemctl disable --now dnf-makecache.timer 2>/dev/null || true
    dnf -y upgrade --refresh
    dnf -y group install --with-optional workstation-product-environment || \
        dnf -y group install workstation-product-environment
    # Identify as Workstation like an installed system (os-release, presets, firewalld zone).
    dnf -y swap fedora-release-identity-cloud fedora-release-identity-workstation --allowerasing
    dnf -y swap fedora-release-cloud fedora-release-workstation --allowerasing || true
    dnf -y upgrade 'fedora-release*'
    if [[ -e /etc/firewalld/firewalld-workstation.conf ]]; then
        ln -sf firewalld-workstation.conf /etc/firewalld/firewalld.conf
    fi
    dnf -y install gnome-keyring libsecret gnome-shell-extension-appindicator qemu-guest-agent \
        openssh-server tcpdump wireshark-cli nftables wireguard-tools bind-utils curl conntrack-tools \
        jq iproute iproute-tc kernel-modules-extra python3 NetworkManager systemd-resolved
    # gsettings for GNOME Software: no automatic downloads (deviation).
    install -d /etc/dconf/db/local.d /etc/dconf/profile
    grep -qx 'system-db:local' /etc/dconf/profile/user 2>/dev/null || \
        printf 'user-db:user\nsystem-db:local\n' > /etc/dconf/profile/user
    cat > /etc/dconf/db/local.d/00-wave-lab <<'EOF'
[org/gnome/desktop/session]
idle-delay=uint32 0

[org/gnome/desktop/screensaver]
lock-enabled=false

[org/gnome/settings-daemon/plugins/power]
sleep-inactive-ac-type='nothing'

[org/gnome/shell]
welcome-dialog-last-shown-version='9999'

[org/gnome/software]
download-updates=false
download-updates-notify=false
EOF
    dconf update

    # NetworkManager creates the default wired profile itself, as after an install.
    rm -f /etc/NetworkManager/system-connections/cloud-init-*.nmconnection
    systemctl enable NetworkManager.service systemd-resolved.service
    touch /etc/cloud/cloud-init.disabled

    # Account: wheel, password login, passwordless sudo for automation (deviation).
    usermod -aG wheel "$user"
    usermod -c 'Wave Lab' "$user"
    if [[ -s /root/.wave-lab-password ]]; then
        printf '%s:%s\n' "$user" "$(head -n1 /root/.wave-lab-password)" | chpasswd
        rm -f /root/.wave-lab-password
    fi
    passwd -u "$user" >/dev/null 2>&1 || true
    printf '%s ALL=(ALL) NOPASSWD:ALL\n' "$user" > /etc/sudoers.d/90-wave-lab
    chmod 440 /etc/sudoers.d/90-wave-lab
    rm -f /etc/sudoers.d/90-cloud-init-users

    install -d /etc/gdm
    printf '[daemon]\nAutomaticLoginEnable=true\nAutomaticLogin=%s\n\n[security]\n\n[xdmcp]\n\n[chooser]\n\n[debug]\n' \
        "$user" > /etc/gdm/custom.conf
    home=$(getent passwd "$user" | cut -d: -f6)
    install -d -o "$user" -g "$user" "$home/.config"
    echo yes > "$home/.config/gnome-initial-setup-done"
    chown "$user:$user" "$home/.config/gnome-initial-setup-done"

    # Recovery channels. Fedora filters guest-agent RPCs and confines guest-exec;
    # the lab needs both (deviation).
    printf 'FILTER_RPC_ARGS=""\nBLACKLIST_RPC=""\n' > /etc/sysconfig/qemu-ga
    # guest-exec children inherit the confined virt_qemu_ga_t domain; make only that
    # domain permissive (SELinux stays enforcing for everything else).
    dnf -y install policycoreutils-python-utils
    semanage permissive -a virt_qemu_ga_t
    systemctl enable qemu-guest-agent.service sshd.service serial-getty@ttyS0.service
    install -d -m 2755 -g systemd-journal /var/log/journal

    cat > /usr/local/sbin/wave-lab-firstboot <<'EOF'
#!/bin/sh
set -eu
[ -s /etc/ssh/ssh_host_ed25519_key ] || ssh-keygen -A
name=$(cat /sys/class/dmi/id/product_serial 2>/dev/null || true)
if printf '%s' "$name" | grep -Eqx 'wave-[a-z0-9-]{1,48}' && [ "$(cat /etc/hostname 2>/dev/null)" != "$name" ]; then
    echo "$name" > /etc/hostname
    hostname "$name"
fi
EOF
    chmod 755 /usr/local/sbin/wave-lab-firstboot
    restorecon /usr/local/sbin/wave-lab-firstboot 2>/dev/null || true
    cat > /etc/systemd/system/wave-lab-firstboot.service <<'EOF'
[Unit]
Description=Wave lab per-clone identity (SSH host keys, hostname)
DefaultDependencies=no
After=local-fs.target
Before=sshd.service NetworkManager.service network-pre.target
Wants=network-pre.target

[Service]
Type=oneshot
ExecStart=/usr/local/sbin/wave-lab-firstboot

[Install]
WantedBy=sysinit.target
EOF
    systemctl enable wave-lab-firstboot.service
    systemctl set-default graphical.target
    echo "provisioned $(. /etc/os-release; echo "$PRETTY_NAME")"
}

seal_phase() {
    dnf -y autoremove || true
    dnf clean all
    home=$(getent passwd "$user" | cut -d: -f6)
    rm -rf "$home/.local/share/keyrings" "$home/.cache" "$home/.bash_history" /root/.bash_history
    rm -f /etc/ssh/ssh_host_*
    rm -rf /var/lib/NetworkManager/internal-*.lease /var/lib/NetworkManager/secret_key /var/lib/NetworkManager/timestamps
    rm -f /var/lib/systemd/random-seed /var/tmp/wave-provision.sh
    cloud-init clean --logs --seed >/dev/null 2>&1 || true
    journalctl --rotate >/dev/null 2>&1 || true
    journalctl --vacuum-time=1s >/dev/null 2>&1 || true
    truncate -s 0 /etc/machine-id
    fstrim -av || true
    sync
    echo sealed
}

case $phase in
    install) install_phase ;;
    seal) seal_phase ;;
    *) echo "unknown phase $phase" >&2; exit 2 ;;
esac
