#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Turn the official Arch Linux cloud image into a KDE Plasma desktop test
# template shaped like an archinstall "KDE Plasma" profile install.
# Run as root inside a dedicated disposable guest: arch-provision.sh install|seal
# Environment: WAVE_LAB_USER (default wave-lab). WAVE_APT_PROXY is ignored here.
# The login password is read once from /root/.wave-lab-password and deleted.
set -euo pipefail
user=${WAVE_LAB_USER:-wave-lab}
phase=${1:?usage: arch-provision.sh install|seal}
pac=(pacman --noconfirm --needed)

install_phase() {
    cloud-init status --wait >/dev/null 2>&1 || true
    # The image initialises its pacman keyring on first boot.
    systemctl start pacman-init.service 2>/dev/null || true
    pacman --noconfirm -Sy archlinux-keyring
    pacman --noconfirm -Su
    # archinstall's KDE Plasma profile (plasma-meta, konsole, kwrite, dolphin, ark,
    # sddm), PipeWire audio and NetworkManager, plus the lab tools.
    "${pac[@]}" -S plasma-meta plasma-workspace konsole kwrite dolphin ark egl-wayland sddm \
        pipewire pipewire-alsa pipewire-pulse pipewire-jack wireplumber networkmanager zram-generator \
        kwallet-pam kwalletmanager libsecret qemu-guest-agent openssh sudo \
        tcpdump wireshark-cli nftables wireguard-tools bind curl conntrack-tools jq iproute2 python

    # Locale as archinstall sets it by default.
    sed -i 's/^#\(en_US.UTF-8 UTF-8\)/\1/' /etc/locale.gen
    locale-gen
    printf 'LANG=en_US.UTF-8\n' > /etc/locale.conf
    printf 'KEYMAP=us\n' > /etc/vconsole.conf

    # NetworkManager owns the NIC. Arch has no default resolver stack; we keep
    # systemd-resolved (enabled in the cloud image) in stub mode, which
    # NetworkManager detects and feeds over D-Bus, as the Arch wiki describes.
    systemctl disable systemd-networkd.service systemd-networkd.socket systemd-networkd-wait-online.service 2>/dev/null || true
    rm -f /etc/systemd/network/*.network
    systemctl enable NetworkManager.service systemd-resolved.service sshd.service
    ln -sf ../run/systemd/resolve/stub-resolv.conf /etc/resolv.conf
    # archinstall's default swap: zram.
    printf '[zram0]\nzram-size = min(ram / 2, 4096)\ncompression-algorithm = zstd\n' > /etc/systemd/zram-generator.conf

    # Account: wheel, password login, passwordless sudo for automation (deviation).
    usermod -aG wheel "$user"
    usermod -c 'Wave Lab' "$user"
    if [[ -s /root/.wave-lab-password ]]; then
        printf '%s:%s\n' "$user" "$(head -n1 /root/.wave-lab-password)" | chpasswd
        rm -f /root/.wave-lab-password
    fi
    passwd -u "$user" >/dev/null 2>&1 || true
    install -d -m 750 /etc/sudoers.d
    printf '%s ALL=(ALL) NOPASSWD:ALL\n' "$user" > /etc/sudoers.d/90-wave-lab
    chmod 440 /etc/sudoers.d/90-wave-lab
    rm -f /etc/sudoers.d/90-cloud-init-users

    # SDDM with Plasma (Wayland) and autologin (switch with labctl login-mode).
    install -d /etc/sddm.conf.d
    # Plasma's Wayland session file is plasma.desktop upstream, plasmawayland.desktop on openSUSE.
    session=plasma
    [[ -e /usr/share/wayland-sessions/plasma.desktop ]] || session=plasmawayland
    printf '[Autologin]\nUser=%s\nSession=%s\nRelogin=false\n' "$user" "$session" > /etc/sddm.conf.d/zz-wave-lab-autologin.conf
    systemctl enable sddm.service
    systemctl set-default graphical.target

    # Unlocked, never-sleeping screen for screenshots; no welcome or update
    # pop-ups over the screen under test (deviations; tests may change them).
    install -d /etc/xdg
    printf '[Daemon]\nAutolock=false\nLockOnResume=false\nTimeout=0\n' > /etc/xdg/kscreenlockerrc
    cat > /etc/xdg/powerdevilrc <<'EOF'
[AC][Display]
DimDisplayWhenIdle=false
TurnOffDisplayWhenIdle=false

[AC][SuspendAndShutdown]
AutoSuspendAction=0
EOF
    home=$(getent passwd "$user" | cut -d: -f6)
    install -d -o "$user" -g "$(id -gn "$user")" "$home/.config" "$home/.config/autostart"
    printf '[Desktop Entry]\nType=Application\nName=org.kde.discover.notifier\nHidden=true\n' \
        > "$home/.config/autostart/org.kde.discover.notifier.desktop"
    # Plasma's Welcome Center (started by a kded module) records the version it was shown for.
    welcome=$(pacman -Q plasma-welcome | awk '{print $2}')
    printf '[General]\nLastSeenVersion=%s\n' "${welcome%%-*}" > "$home/.config/plasma-welcomerc"
    chown -R "$user:$(id -gn "$user")" "$home/.config"

    # Recovery channels: guest agent and a serial getty.
    systemctl enable qemu-guest-agent.service serial-getty@ttyS0.service 2>/dev/null || true
    # Cloud-image-only boot settings: classic NIC names, waiting for NTP before
    # time-sync.target (holds sshd for a minute) and a keyring init on every
    # first boot (the template's keyring is already initialised).
    systemctl disable systemd-time-wait-sync.service pacman-init.service 2>/dev/null || true
    rm -rf /etc/systemd/system/cloud-init.target.wants
    if [[ -e /etc/default/grub ]]; then
        sed -i -e 's/^GRUB_TIMEOUT=.*/GRUB_TIMEOUT=1/' -e 's/net\.ifnames=0 *//' /etc/default/grub
        grep -q 'console=ttyS0' /etc/default/grub || \
            sed -i 's/^GRUB_CMDLINE_LINUX_DEFAULT="/GRUB_CMDLINE_LINUX_DEFAULT="console=ttyS0,115200 console=tty1 /' /etc/default/grub
        grub-mkconfig -o /boot/grub/grub.cfg
    fi
    install -d -m 2755 -g systemd-journal /var/log/journal
    install -d /etc/systemd/journald.conf.d
    printf '[Journal]\nStorage=persistent\n' > /etc/systemd/journald.conf.d/50-wave-lab.conf

    cat > /usr/local/sbin/wave-lab-firstboot <<'EOF'
#!/bin/sh
set -eu
[ -s /etc/ssh/ssh_host_ed25519_key ] || ssh-keygen -A
name=$(cat /sys/class/dmi/id/product_serial 2>/dev/null || true)
if printf '%s' "$name" | grep -Eqx 'wave-[a-z0-9-]{1,48}' && [ "$(cat /etc/hostname 2>/dev/null)" != "$name" ]; then
    echo "$name" > /etc/hostname
    echo "$name" > /proc/sys/kernel/hostname  # no hostname(1) without inetutils
fi
EOF
    chmod 755 /usr/local/sbin/wave-lab-firstboot
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
    # Installed desktops have no cloud-init.
    pacman --noconfirm -Rns cloud-init 2>/dev/null || true
    rm -rf /etc/cloud /var/lib/cloud
    echo "provisioned $(. /etc/os-release; echo "$PRETTY_NAME") $(uname -r) -> $(pacman -Q linux)"
}

seal_phase() {
    pacman --noconfirm -Scc >/dev/null
    home=$(getent passwd "$user" | cut -d: -f6)
    rm -rf "$home/.local/share/kwalletd" "$home/.local/share/keyrings" "$home/.cache" \
        "$home/.bash_history" /root/.bash_history
    rm -f /etc/ssh/ssh_host_*
    rm -rf /var/lib/NetworkManager/internal-*.lease /var/lib/NetworkManager/secret_key /var/lib/NetworkManager/timestamps
    rm -f /var/lib/systemd/random-seed /var/tmp/wave-provision.sh
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
