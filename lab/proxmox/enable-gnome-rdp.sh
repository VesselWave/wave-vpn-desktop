#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Run as root inside the dedicated Ubuntu GNOME guest.
set -euo pipefail
test "$(id -u)" = 0
daemon_home=$(getent passwd gnome-remote-desktop | cut -d: -f6)
test -n "$daemon_home"
cert_dir="$daemon_home/.local/share/gnome-remote-desktop"
install -d -m 700 -o gnome-remote-desktop -g gnome-remote-desktop "$cert_dir"
if [ ! -s "$cert_dir/rdp-tls.key" ]; then
  openssl req -x509 -newkey rsa:3072 -nodes -days 365 \
    -subj /CN=wave-ubuntu-gnome \
    -keyout "$cert_dir/rdp-tls.key" -out "$cert_dir/rdp-tls.crt" 2>/dev/null
  chown gnome-remote-desktop:gnome-remote-desktop "$cert_dir/rdp-tls.key" "$cert_dir/rdp-tls.crt"
  chmod 600 "$cert_dir/rdp-tls.key"
fi
install -d -m 700 -o wave-lab -g wave-lab /home/wave-lab/.config/wave-lab
password_file=/home/wave-lab/.config/wave-lab/rdp-password
if [ ! -s "$password_file" ]; then
  (umask 077; openssl rand -base64 24 > "$password_file")
  chown wave-lab:wave-lab "$password_file"
fi
grdctl --system rdp set-tls-key "$cert_dir/rdp-tls.key"
grdctl --system rdp set-tls-cert "$cert_dir/rdp-tls.crt"
grdctl --system rdp set-credentials wave-lab "$(cat "$password_file")"
grdctl --system rdp enable
systemctl enable --now gnome-remote-desktop.service
systemctl restart gnome-remote-desktop.service
systemctl is-active gnome-remote-desktop.service
timeout 20 bash -c 'until ss -H -ltn "sport = :3389" | grep -q .; do sleep 1; done'
ss -ltn 'sport = :3389'
