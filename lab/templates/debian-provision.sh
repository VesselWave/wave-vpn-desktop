#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Turn a Debian 13 "generic" cloud image into a GNOME desktop test template that
# matches a debian-installer install with the "GNOME" and "Debian desktop" tasks.
# Run as root inside a dedicated disposable guest: debian-provision.sh install|seal
# Environment: WAVE_LAB_USER (default wave-lab), WAVE_APT_PROXY (optional).
# The login password is read once from /root/.wave-lab-password and deleted.
set -euo pipefail
user=${WAVE_LAB_USER:-wave-lab}
phase=${1:?usage: debian-provision.sh install|seal}
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
    # tasksel's "Debian desktop environment" + "GNOME" tasks (Recommends included, as tasksel does).
    # The generic image already runs the standard linux-image-amd64 kernel (DRM, sch_netem).
    apt-get "${apt_opts[@]}" install task-desktop task-gnome-desktop ifupdown \
        gnome-keyring libsecret-tools network-manager openssh-server qemu-guest-agent \
        tcpdump tshark nftables wireguard-tools bind9-dnsutils curl conntrack jq iproute2 python3
    apt-get "${apt_opts[@]}" purge unattended-upgrades || true

    # Account: d-i desktop groups, password login, passwordless sudo for automation (deviation).
    for group in cdrom floppy sudo audio dip video plugdev users netdev bluetooth lpadmin scanner; do
        getent group "$group" >/dev/null && usermod -aG "$group" "$user"
    done
    usermod -c 'Wave Lab' "$user"
    if [[ -s /root/.wave-lab-password ]]; then
        printf '%s:%s\n' "$user" "$(head -n1 /root/.wave-lab-password)" | chpasswd
        rm -f /root/.wave-lab-password
    fi
    passwd -u "$user" >/dev/null 2>&1 || true
    printf '%s ALL=(ALL) NOPASSWD:ALL\n' "$user" > /etc/sudoers.d/90-wave-lab
    chmod 440 /etc/sudoers.d/90-wave-lab
    rm -f /etc/sudoers.d/90-cloud-init-users

    # GDM autologin (switch with labctl login-mode). Debian's gdm3 reads daemon.conf.
    printf '[daemon]\nAutomaticLoginEnable=true\nAutomaticLogin=%s\n\n[security]\n\n[xdmcp]\n\n[chooser]\n\n[debug]\n' \
        "$user" > /etc/gdm3/daemon.conf
    home=$(getent passwd "$user" | cut -d: -f6)
    install -d -o "$user" -g "$user" "$home/.config"
    echo yes > "$home/.config/gnome-initial-setup-done"
    chown "$user:$user" "$home/.config/gnome-initial-setup-done"
    # Screenshots need an unlocked, unblanked screen (deviation; tests may change it).
    install -d /etc/dconf/profile /etc/dconf/db/local.d
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

    # Recovery channels: guest agent and a serial getty.
    systemctl enable qemu-guest-agent.service 2>/dev/null || true
    systemctl enable serial-getty@ttyS0.service
    printf 'GRUB_CMDLINE_LINUX=""\nGRUB_CMDLINE_LINUX_DEFAULT="console=ttyS0,115200 console=tty1 quiet"\nGRUB_TIMEOUT=1\n' \
        > /etc/default/grub.d/99-wave-lab.cfg
    update-grub

    # Persistent journal and swap (d-i creates a swap partition; a file is equivalent here).
    install -d -m 2755 -g systemd-journal /var/log/journal
    install -d /etc/systemd/journald.conf.d
    printf '[Journal]\nStorage=persistent\n' > /etc/systemd/journald.conf.d/50-wave-lab.conf
    if [[ ! -e /swapfile ]]; then
        fallocate -l 2G /swapfile
        chmod 600 /swapfile
        mkswap /swapfile >/dev/null
        echo '/swapfile none swap sw 0 0' >> /etc/fstab
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
    systemctl set-default graphical.target

    # Networking last, because it removes the resolver this run still uses.
    # A d-i desktop install has no netplan, networkd or resolved: NetworkManager
    # owns the NIC, ifupdown only knows lo, and /etc/resolv.conf is a plain file
    # that NetworkManager writes.
    printf '# This file describes the network interfaces available on your system\n# and how to activate them. For more information, see interfaces(5).\n\nsource /etc/network/interfaces.d/*\n\n# The loopback network interface\nauto lo\niface lo inet loopback\n' \
        > /etc/network/interfaces
    rm -f /etc/netplan/*.yaml
    systemctl disable systemd-networkd.service systemd-networkd.socket systemd-networkd-wait-online.service 2>/dev/null || true
    systemctl enable NetworkManager.service
    apt-get "${apt_opts[@]}" purge cloud-init netplan.io netplan-generator python3-netplan systemd-resolved || true
    rm -rf /etc/cloud /var/lib/cloud
    rm -f /etc/resolv.conf
    echo "provisioned $(. /etc/os-release; echo "$PRETTY_NAME") ($(cat /etc/debian_version))"
}

seal_phase() {
    apt-get "${apt_opts[@]}" autoremove --purge
    apt-get clean
    home=$(getent passwd "$user" | cut -d: -f6)
    rm -rf "$home/.local/share/keyrings" "$home/.cache" "$home/.bash_history" /root/.bash_history
    rm -f /etc/ssh/ssh_host_*
    rm -f /var/lib/NetworkManager/*.lease /var/lib/NetworkManager/secret_key /var/lib/NetworkManager/timestamps
    rm -f /var/lib/systemd/random-seed /var/tmp/wave-provision.sh
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
