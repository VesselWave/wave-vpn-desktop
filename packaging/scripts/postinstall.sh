#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Runs after the Wave VPN package is installed or upgraded.
# The networking helper is a root-owned copy of wave-vpn, so refresh it.
bin=/usr/bin/wave-vpn
config=/etc/wave-vpn/service.json
if [ -d /run/systemd/system ]; then
    if [ -e /run/wave-vpn/recovery.json ]; then
        echo 'Wave VPN is connected. Disconnect, then run: sudo wave-vpn service-install'
    elif [ -f "$config" ]; then
        uid=$(sed -n 's/.*"uid":\([0-9][0-9]*\).*/\1/p' "$config")
        if [ -z "$uid" ] || ! "$bin" service-install --uid "$uid" > /dev/null; then
            echo 'Wave VPN: the networking helper will be set up again when you press Connect.'
        fi
    elif [ "${SUDO_UID:-0}" != 0 ]; then
        # Installed with sudo from a terminal: authorize that desktop user now
        # instead of asking for a password on the first Connect.
        "$bin" service-install > /dev/null 2>&1 || true
    fi
    if [ -f /etc/systemd/system/wave-vpn-lock.service ]; then
        install -m 755 "$bin" /usr/local/libexec/wave-vpn-service
    fi
fi
if [ -e /usr/local/bin/wave-vpn ]; then
    echo 'An older Wave VPN is installed in /usr/local and runs instead of this package.'
    echo 'Remove it with: sudo wave-vpn-uninstall (or the install.sh uninstall it came with)'
fi
exit 0
