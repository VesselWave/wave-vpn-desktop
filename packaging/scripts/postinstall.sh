#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Runs after the Wave VPN package is installed or upgraded.
# The networking helper is a root-owned copy of wave-vpn, so refresh it.
# WAVE_PACKAGE_TEST_ROOT is only for scripts/tests/test_packaging_scripts.py.
root=${WAVE_PACKAGE_TEST_ROOT:-}
bin=$root/usr/bin/wave-vpn
config=$root/etc/wave-vpn/service.json
if [ -d "$root/run/systemd/system" ]; then
    if [ -f "$config" ]; then
        # Also while connected: service-install then replaces the helper
        # without stopping the running connection, which finishes on the old
        # one; the next connection uses the new helper.
        uid=$(sed -n 's/.*"uid":\([0-9][0-9]*\).*/\1/p' "$config")
        if [ -z "$uid" ] || ! "$bin" service-install --uid "$uid" > /dev/null; then
            echo 'Wave VPN: could not update the networking helper. Disconnect, then run: sudo wave-vpn service-install'
        fi
    elif [ "${SUDO_UID:-0}" != 0 ]; then
        # Installed with sudo from a terminal: authorize that desktop user now
        # instead of asking for a password on the first Connect.
        "$bin" service-install > /dev/null 2>&1 || true
    fi
    if [ -f "$root/etc/systemd/system/wave-vpn-lock.service" ]; then
        install -m 755 "$bin" "$root/usr/local/libexec/wave-vpn-service"
    fi
fi
if [ -e "$root/usr/local/bin/wave-vpn" ]; then
    echo 'An older Wave VPN is installed in /usr/local and runs instead of this package.'
    echo 'Remove it with: sudo wave-vpn-uninstall (or the install.sh uninstall it came with)'
fi
exit 0
