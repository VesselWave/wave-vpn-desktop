#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Turn an Ubuntu LTS cloud image into a desktop test template.
# Run as root inside a dedicated disposable guest: ubuntu-provision.sh install|seal
# Environment: WAVE_LAB_USER (default wave-lab), WAVE_APT_PROXY (optional).
# The login password is read once from /root/.wave-lab-password and deleted.
set -euo pipefail
user=${WAVE_LAB_USER:-wave-lab}
phase=${1:?usage: ubuntu-provision.sh install|seal}
export DEBIAN_FRONTEND=noninteractive
apt_opts=(-y -o DPkg::Lock::Timeout=900 -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold)

install_phase() {
    cloud-init status --wait >/dev/null 2>&1 || true
    rm -f /etc/apt/apt.conf.d/00wave-lab-proxy
    if [[ -n ${WAVE_APT_PROXY:-} ]]; then
        hostport=${WAVE_APT_PROXY#*://}
        hostport=${hostport%%/*}
        if timeout 5 bash -c "</dev/tcp/${hostport%:*}/${hostport##*:}" 2>/dev/null; then
            printf 'Acquire::http::Proxy "%s";\n' "$WAVE_APT_PROXY" > /etc/apt/apt.conf.d/00wave-lab-proxy
        else
            echo "warning: apt proxy $WAVE_APT_PROXY unreachable; using the distribution mirrors" >&2
        fi
    fi
    # Deviation from a stock install: no background package activity during tests.
    systemctl disable --now apt-daily.timer apt-daily-upgrade.timer 2>/dev/null || true
    systemctl mask apt-daily.service apt-daily-upgrade.service unattended-upgrades.service
    cat > /etc/apt/apt.conf.d/99wave-lab-no-periodic <<'EOF'
APT::Periodic::Update-Package-Lists "0";
APT::Periodic::Download-Upgradeable-Packages "0";
APT::Periodic::AutocleanInterval "0";
APT::Periodic::Unattended-Upgrade "0";
EOF
    echo 'wireshark-common wireshark-common/install-setuid boolean false' | debconf-set-selections
    apt-get -o DPkg::Lock::Timeout=900 --error-on=any update
    apt-get "${apt_opts[@]}" full-upgrade
    # shellcheck disable=SC1091
    . /etc/os-release
    kernel=linux-generic
    # 24.04 point-release desktop media install the HWE kernel.
    [[ $VERSION_ID == 24.04 ]] && kernel=linux-generic-hwe-24.04
    apt-get "${apt_opts[@]}" install ubuntu-desktop "$kernel" gnome-shell-extension-appindicator \
        gnome-keyring libsecret-tools network-manager systemd-resolved openssh-server qemu-guest-agent \
        tcpdump tshark nftables wireguard-tools dnsutils curl conntrack jq iproute2 python3
    # The meta package pulls linux-modules-extra (sch_netem and friends) for the new kernel.
    apt-get "${apt_opts[@]}" purge unattended-upgrades || true

    # NetworkManager owns every device, exactly like the desktop installer's netplan stub.
    rm -f /etc/netplan/50-cloud-init.yaml
    printf '# Let NetworkManager manage all devices on this system\nnetwork:\n  version: 2\n  renderer: NetworkManager\n' \
        > /etc/netplan/01-network-manager-all.yaml
    chmod 600 /etc/netplan/01-network-manager-all.yaml
    systemctl disable systemd-networkd.service systemd-networkd.socket systemd-networkd-wait-online.service 2>/dev/null || true
    systemctl mask systemd-networkd-wait-online.service
    systemctl enable NetworkManager.service systemd-resolved.service
    # Installed desktops ship cloud-init disabled.
    printf 'network: {config: disabled}\n' > /etc/cloud/cloud.cfg.d/99-disable-network-config.cfg
    touch /etc/cloud/cloud-init.disabled

    # Account: desktop groups, password login, passwordless sudo for automation (deviation).
    usermod -aG adm,cdrom,sudo,dip,plugdev,lpadmin,users "$user"
    usermod -c 'Wave Lab' "$user"
    if [[ -s /root/.wave-lab-password ]]; then
        printf '%s:%s\n' "$user" "$(head -n1 /root/.wave-lab-password)" | chpasswd
        rm -f /root/.wave-lab-password
    fi
    passwd -u "$user" >/dev/null 2>&1 || true
    printf '%s ALL=(ALL) NOPASSWD:ALL\n' "$user" > /etc/sudoers.d/90-wave-lab
    chmod 440 /etc/sudoers.d/90-wave-lab
    rm -f /etc/sudoers.d/90-cloud-init-users

    # GDM autologin (switch with labctl login-mode), Wayland stays the default.
    install -d /etc/gdm3
    printf '[daemon]\nAutomaticLoginEnable=true\nAutomaticLogin=%s\n\n[security]\n\n[xdmcp]\n\n[chooser]\n\n[debug]\n' \
        "$user" > /etc/gdm3/custom.conf
    home=$(getent passwd "$user" | cut -d: -f6)
    install -d -o "$user" -g "$user" "$home/.config"
    echo yes > "$home/.config/gnome-initial-setup-done"
    chown "$user:$user" "$home/.config/gnome-initial-setup-done"
    # Screenshots need an unlocked, unblanked screen (deviation; tests may change it).
    install -d /etc/dconf/profile /etc/dconf/db/local.d
    printf 'user-db:user\nsystem-db:local\n' > /etc/dconf/profile/user
    cat > /etc/dconf/db/local.d/00-wave-lab <<'EOF'
[org/gnome/desktop/session]
idle-delay=uint32 0

[org/gnome/desktop/screensaver]
lock-enabled=false

[org/gnome/settings-daemon/plugins/power]
sleep-inactive-ac-type='nothing'
EOF
    dconf update
    # Crash dialogs would cover the screen under test.
    sed -i 's/^enabled=1/enabled=0/' /etc/default/apport 2>/dev/null || true

    # Recovery channels: guest agent and a serial getty.
    systemctl enable qemu-guest-agent.service 2>/dev/null || true
    systemctl enable serial-getty@ttyS0.service
    printf 'GRUB_CMDLINE_LINUX_DEFAULT="console=ttyS0,115200 console=tty1 quiet splash"\nGRUB_TIMEOUT=1\nGRUB_TIMEOUT_STYLE=menu\n' \
        > /etc/default/grub.d/99-wave-lab.cfg
    update-grub

    # Persistent journal and a swap file like the installer creates.
    install -d -m 2755 -g systemd-journal /var/log/journal
    install -d /etc/systemd/journald.conf.d
    printf '[Journal]\nStorage=persistent\n' > /etc/systemd/journald.conf.d/50-wave-lab.conf
    if [[ ! -e /swap.img ]]; then
        fallocate -l 2G /swap.img
        chmod 600 /swap.img
        mkswap /swap.img >/dev/null
        echo '/swap.img none swap sw 0 0' >> /etc/fstab
    fi

    # Per-clone identity: SSH host keys and the hostname from the SMBIOS serial.
    cat > /usr/local/sbin/wave-lab-firstboot <<'EOF'
#!/bin/sh
set -eu
[ -s /etc/ssh/ssh_host_ed25519_key ] || ssh-keygen -A
name=$(cat /sys/class/dmi/id/product_serial 2>/dev/null || true)
if printf '%s' "$name" | grep -Eqx 'wave-[a-z0-9-]{1,48}' && [ "$(cat /etc/hostname)" != "$name" ]; then
    echo "$name" > /etc/hostname
    hostname "$name"
    sed -i '/^127\.0\.1\.1[[:space:]]/d' /etc/hosts
    printf '127.0.1.1 %s\n' "$name" >> /etc/hosts
fi
EOF
    chmod 755 /usr/local/sbin/wave-lab-firstboot
    cat > /etc/systemd/system/wave-lab-firstboot.service <<'EOF'
[Unit]
Description=Wave lab per-clone identity (SSH host keys, hostname)
DefaultDependencies=no
After=local-fs.target
Before=ssh.socket ssh.service sshd.service NetworkManager.service network-pre.target
Wants=network-pre.target

[Service]
Type=oneshot
ExecStart=/usr/local/sbin/wave-lab-firstboot

[Install]
WantedBy=sysinit.target
EOF
    systemctl enable wave-lab-firstboot.service
    snap refresh --hold >/dev/null 2>&1 || true
    systemctl set-default graphical.target
    echo "provisioned $(. /etc/os-release; echo "$PRETTY_NAME")"
}

seal_phase() {
    # Keep only the desktop kernel flavour once it is the running one.
    if dpkg-query -W -f='${Status}' linux-generic-hwe-24.04 2>/dev/null | grep -q 'install ok installed'; then
        for meta in linux-virtual linux-image-virtual linux-headers-virtual linux-generic linux-image-generic linux-headers-generic; do
            if dpkg-query -W -f='${Status}' "$meta" 2>/dev/null | grep -q 'install ok installed'; then
                apt-get "${apt_opts[@]}" purge "$meta"
            fi
        done
    fi
    apt-get "${apt_opts[@]}" autoremove --purge
    apt-get clean
    home=$(getent passwd "$user" | cut -d: -f6)
    rm -rf "$home/.local/share/keyrings" "$home/.cache" "$home/.bash_history" /root/.bash_history
    rm -f /etc/ssh/ssh_host_*
    rm -f /var/lib/NetworkManager/*.lease /var/lib/NetworkManager/secret_key /var/lib/NetworkManager/timestamps
    rm -f /var/lib/systemd/random-seed /var/tmp/wave-provision.sh
    cloud-init clean --logs --seed >/dev/null 2>&1 || true
    journalctl --rotate >/dev/null 2>&1 || true
    journalctl --vacuum-time=1s >/dev/null 2>&1 || true
    find /var/log -type f \( -name '*.gz' -o -name '*.[0-9]' \) -delete
    find /var/log -type f -name '*.log' -exec truncate -s 0 {} +
    truncate -s 0 /etc/machine-id
    rm -f /var/lib/dbus/machine-id
    ln -s /etc/machine-id /var/lib/dbus/machine-id
    fstrim -av || true
    sync
    echo sealed
}

case $phase in
    install) install_phase ;;
    seal) seal_phase ;;
    *) echo "unknown phase $phase" >&2; exit 2 ;;
esac
