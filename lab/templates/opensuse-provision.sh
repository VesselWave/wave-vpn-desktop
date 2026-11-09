#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Turn an openSUSE Tumbleweed Minimal-VM cloud image into a KDE Plasma desktop
# test template that matches the installer's KDE desktop selection.
# Run as root inside a dedicated disposable guest: opensuse-provision.sh install|seal
# Environment: WAVE_LAB_USER (default wave-lab). WAVE_APT_PROXY is ignored here.
# The login password is read once from /root/.wave-lab-password and deleted.
set -euo pipefail
user=${WAVE_LAB_USER:-wave-lab}
phase=${1:?usage: opensuse-provision.sh install|seal}
zyp=(zypper --non-interactive --gpg-auto-import-keys)

install_phase() {
    cloud-init status --wait >/dev/null 2>&1 || true
    # Minimal-VM ships zypp drop-ins that skip recommended packages and docs;
    # installed desktops have neither, so mask them with same-named /etc files.
    install -d /etc/zypp/zypp.conf.d
    printf '[main]\nsolver.onlyRequires = false\n' > /etc/zypp/zypp.conf.d/no-recommends.conf
    printf '[main]\nrpm.install.excludedocs = no\n' > /etc/zypp/zypp.conf.d/excludedocs.conf
    "${zyp[@]}" refresh
    "${zyp[@]}" dist-upgrade --auto-agree-with-licenses
    # The installer's KDE role: Plasma, KDE applications, X11 fallback and the
    # enhanced base system, with recommended packages (zypper's default).
    "${zyp[@]}" install --auto-agree-with-licenses -t pattern kde kde_plasma x11 enhanced_base
    # Recommends of everything already installed, as if they had been installed together.
    "${zyp[@]}" install-new-recommends
    # The image boots kernel-default-base (no DRM drivers, no netem); installs use kernel-default.
    if rpm -q kernel-default-base >/dev/null 2>&1; then
        "${zyp[@]}" install --auto-agree-with-licenses -- kernel-default -kernel-default-base || {
            "${zyp[@]}" install --auto-agree-with-licenses kernel-default
            "${zyp[@]}" remove kernel-default-base || true
        }
    fi
    "${zyp[@]}" install --auto-agree-with-licenses sddm-qt6 firewalld NetworkManager pam_kwallet6 kwalletmanager \
        qemu-guest-agent openssh-server policycoreutils-python-utils tcpdump nftables jq curl python3 \
        secret-tool wireshark wireguard-tools bind-utils conntrack-tools iproute2
    # Deviation: no background package activity during tests.
    for unit in packagekit-background.timer zypper-refresh.timer snapper-timeline.timer; do
        systemctl disable --now "$unit" 2>/dev/null || true
    done

    # Desktop installs: NetworkManager (not wicked), firewalld in its default
    # "public" zone (ssh allowed), resolv.conf via netconfig.
    rm -f /etc/NetworkManager/system-connections/cloud-init-*.nmconnection
    systemctl enable NetworkManager.service firewalld.service sshd.service
    systemctl disable wicked.service 2>/dev/null || true
    firewall-offline-cmd --zone=public --query-service=ssh >/dev/null 2>&1 || \
        firewall-offline-cmd --zone=public --add-service=ssh

    # Account: password login, passwordless sudo for automation (deviation).
    usermod -aG users "$user" 2>/dev/null || true
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
    update-alternatives --set default-displaymanager /usr/lib/X11/displaymanagers/sddm 2>/dev/null || true
    if [[ -e /etc/sysconfig/displaymanager ]]; then
        sed -i -e 's/^DISPLAYMANAGER=.*/DISPLAYMANAGER="sddm"/' \
            -e "s/^DISPLAYMANAGER_AUTOLOGIN=.*/DISPLAYMANAGER_AUTOLOGIN=\"$user\"/" /etc/sysconfig/displaymanager
    fi
    install -d /etc/sddm.conf.d
    printf '[Autologin]\nUser=%s\nSession=plasma\nRelogin=false\n' "$user" > /etc/sddm.conf.d/zz-wave-lab-autologin.conf
    # openSUSE starts the alternative's manager through display-manager-legacy.service.
    systemctl enable display-manager-legacy.service 2>/dev/null || systemctl enable sddm.service
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
    for app in org.kde.plasma-welcome org.kde.discover.notifier; do
        printf '[Desktop Entry]\nType=Application\nName=%s\nHidden=true\n' "$app" > "$home/.config/autostart/$app.desktop"
    done
    # opensuse-welcome-launcher shows once per version unless this stamp says it ran.
    install -d "$home/.local/share/opensuse-welcome"
    echo 1 > "$home/.local/share/opensuse-welcome/launched"
    chown -R "$user:$(id -gn "$user")" "$home/.config" "$home/.local"

    # Recovery channels. openSUSE blocks guest-exec in /usr/etc/sysconfig/qemu-ga
    # and guest-exec children inherit the confined virt_qemu_ga_t domain; the lab
    # needs both, so clear the RPC filter and make only that domain permissive
    # (SELinux stays enforcing for everything else). Deviations.
    printf 'FILTER_RPC_ARGS=""\n' > /etc/sysconfig/qemu-ga
    if command -v semanage >/dev/null && selinuxenabled 2>/dev/null; then
        semanage permissive -a virt_qemu_ga_t 2>/dev/null || true
    fi
    systemctl enable qemu-guest-agent.service serial-getty@ttyS0.service 2>/dev/null || true
    if [[ -e /etc/default/grub ]]; then
        sed -i -e 's/^GRUB_TIMEOUT=.*/GRUB_TIMEOUT=1/' /etc/default/grub
        grep -q 'console=ttyS0' /etc/default/grub || \
            sed -i 's/^GRUB_CMDLINE_LINUX_DEFAULT="/GRUB_CMDLINE_LINUX_DEFAULT="console=ttyS0,115200 console=tty1 /' /etc/default/grub
        grub2-mkconfig -o /boot/grub2/grub.cfg
    fi
    install -d -m 2755 -g systemd-journal /var/log/journal
    install -d /etc/systemd/journald.conf.d
    printf '[Journal]\nStorage=persistent\n' > /etc/systemd/journald.conf.d/50-wave-lab.conf
    # Installed systems have a swap partition; a swap file on btrfs is equivalent here.
    if [[ ! -e /swapfile ]]; then
        if [[ $(findmnt -no FSTYPE /) == btrfs ]]; then
            btrfs filesystem mkswapfile --size 2g /swapfile >/dev/null
        else
            fallocate -l 2G /swapfile && chmod 600 /swapfile && mkswap /swapfile >/dev/null
        fi
        echo '/swapfile none swap defaults 0 0' >> /etc/fstab
    fi

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
    restorecon /etc/systemd/system/wave-lab-firstboot.service 2>/dev/null || true
    systemctl enable wave-lab-firstboot.service
    # Installed desktops have no cloud-init.
    "${zyp[@]}" remove --clean-deps cloud-init cloud-init-config-suse 2>/dev/null || true
    rm -rf /etc/cloud /var/lib/cloud
    echo "provisioned $(. /etc/os-release; echo "$PRETTY_NAME $VERSION_ID")"
}

seal_phase() {
    zypper --non-interactive clean --all
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
