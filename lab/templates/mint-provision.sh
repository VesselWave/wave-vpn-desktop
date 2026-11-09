#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Prepare a Linux Mint Cinnamon system installed from the live ISO (labctl's
# automatic Ubiquity install) as a desktop test template. The desktop itself is
# exactly what the installer put there; this only adds lab access and tools.
# Run as root inside a dedicated disposable guest: mint-provision.sh install|seal
# Environment: WAVE_LAB_USER (default wave-lab), WAVE_APT_PROXY (optional).
set -euo pipefail
user=${WAVE_LAB_USER:-wave-lab}
phase=${1:?usage: mint-provision.sh install|seal}
export DEBIAN_FRONTEND=noninteractive
apt_opts=(-y -o DPkg::Lock::Timeout=900 -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold)

install_phase() {
    rm -f /etc/apt/apt.conf.d/00wave-lab-proxy /root/.wave-lab-password
    if [[ -n ${WAVE_APT_PROXY:-} ]]; then
        hostport=${WAVE_APT_PROXY#*://}
        hostport=${hostport%%/*}
        if timeout 5 bash -c "</dev/tcp/${hostport%:*}/${hostport##*:}" 2>/dev/null; then
            printf 'Acquire::http::Proxy "%s";\n' "$WAVE_APT_PROXY" > /etc/apt/apt.conf.d/00wave-lab-proxy
        fi
    fi
    # Deviation: no background package activity during tests (Mint's own
    # automatic updates are already off by default; the Update Manager still
    # refreshes the cache from its tray applet).
    systemctl disable --now apt-daily.timer apt-daily-upgrade.timer 2>/dev/null || true
    systemctl mask apt-daily.service apt-daily-upgrade.service
    for unit in mintupdate-automation-upgrade.timer mintupdate-automation-autoremove.timer; do
        systemctl disable --now "$unit" 2>/dev/null || true
    done
    cat > /etc/apt/apt.conf.d/99wave-lab-no-periodic <<'EOF'
APT::Periodic::Update-Package-Lists "0";
APT::Periodic::Download-Upgradeable-Packages "0";
APT::Periodic::AutocleanInterval "0";
APT::Periodic::Unattended-Upgrade "0";
EOF
    echo 'wireshark-common wireshark-common/install-setuid boolean false' | debconf-set-selections
    apt-get -o DPkg::Lock::Timeout=900 --error-on=any update
    # Same as running the Update Manager once after installing.
    apt-get "${apt_opts[@]}" full-upgrade
    apt-get "${apt_opts[@]}" install gnome-keyring libsecret-tools openssh-server qemu-guest-agent \
        tcpdump tshark nftables wireguard-tools dnsutils curl conntrack jq iproute2 python3
    # The installed generic kernel's linux-modules-extra provides sch_netem.

    usermod -c 'Wave Lab' "$user"
    passwd -u "$user" >/dev/null 2>&1 || true
    printf '%s ALL=(ALL) NOPASSWD:ALL\n' "$user" > /etc/sudoers.d/90-wave-lab
    chmod 440 /etc/sudoers.d/90-wave-lab

    # LightDM autologin (switch with labctl login-mode).
    install -d /etc/lightdm/lightdm.conf.d
    printf '[Seat:*]\nautologin-user=%s\nautologin-user-timeout=0\n' "$user" > /etc/lightdm/lightdm.conf.d/90-wave-lab-autologin.conf
    [[ -e /etc/lightdm/lightdm.conf ]] && sed -i '/^autologin-user=/d' /etc/lightdm/lightdm.conf

    # Unlocked, unblanked screen and no welcome/update pop-ups over the screen
    # under test (deviations; tests may change them).
    install -d /etc/dconf/profile /etc/dconf/db/local.d
    grep -qx 'system-db:local' /etc/dconf/profile/user 2>/dev/null || \
        printf 'user-db:user\nsystem-db:local\n' > /etc/dconf/profile/user
    cat > /etc/dconf/db/local.d/00-wave-lab <<'EOF'
[org/cinnamon/desktop/session]
idle-delay=uint32 0

[org/cinnamon/desktop/screensaver]
lock-enabled=false

[org/cinnamon/settings-daemon/plugins/power]
sleep-display-ac=0
sleep-inactive-ac-timeout=0
lock-on-suspend=false
EOF
    dconf update
    home=$(getent passwd "$user" | cut -d: -f6)
    install -d -o "$user" -g "$user" "$home/.linuxmint/mintwelcome" "$home/.config/autostart"
    : > "$home/.linuxmint/mintwelcome/norun.flag"
    printf '[Desktop Entry]\nType=Application\nName=mintupdate\nHidden=true\n' > "$home/.config/autostart/mintupdate.desktop"
    chown -R "$user:$user" "$home/.linuxmint" "$home/.config"

    # Recovery channels: guest agent and a serial getty.
    systemctl enable qemu-guest-agent.service 2>/dev/null || true
    systemctl enable ssh.service serial-getty@ttyS0.service
    install -d /etc/default/grub.d
    printf 'GRUB_CMDLINE_LINUX_DEFAULT="console=ttyS0,115200 console=tty1 quiet splash"\nGRUB_TIMEOUT=1\nGRUB_TIMEOUT_STYLE=menu\nGRUB_RECORDFAIL_TIMEOUT=1\n' \
        > /etc/default/grub.d/99-wave-lab.cfg
    update-grub
    install -d -m 2755 -g systemd-journal /var/log/journal
    install -d /etc/systemd/journald.conf.d
    printf '[Journal]\nStorage=persistent\n' > /etc/systemd/journald.conf.d/50-wave-lab.conf

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
    echo "provisioned $(. /etc/os-release; echo "$PRETTY_NAME")"
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
