#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Runs before the Wave VPN package is removed. Upgrades keep everything.
# Arguments: Debian "remove" or "upgrade"; RPM 0 for removal, 1 or more for
# upgrade; Arch calls this only for removal, with the package version.
case "${1:-}" in
    upgrade | failed-upgrade | deconfigure | [1-9] | [1-9][0-9]) exit 0 ;;
esac
# WAVE_PACKAGE_TEST_ROOT is only for scripts/tests/test_packaging_scripts.py.
root=${WAVE_PACKAGE_TEST_ROOT:-}
bin=$root/usr/bin/wave-vpn
[ -x "$bin" ] || exit 0
# Close running Wave windows first: once removed, they could only offer to
# connect through a helper that no longer exists, and they must not start a
# connection while it is being taken down.
if command -v pkill > /dev/null 2>&1; then
    pkill -x wave-vpn-gui 2> /dev/null || true
fi
# Nothing to undo unless Wave has set something up since boot or installed units.
state=
for path in /run/wave-vpn /run/wave-vpn-protection.json /etc/systemd/system/wave-vpn-service.socket \
    /etc/systemd/system/wave-vpn-lock.service; do
    if [ -e "$root$path" ]; then state=1; fi
done
[ -n "$state" ] || exit 0
# Never leave a connection or Network Lock behind without the app to undo it.
systemctl stop wave-vpn-service.socket wave-vpn-service.service 2> /dev/null || true
if ! "$bin" lock-disable < /dev/null > /dev/null; then
    echo 'Wave VPN: could not disconnect or turn off Network Lock; run sudo wave-vpn recover.' >&2
    exit 1
fi
if [ -f "$root/etc/systemd/system/wave-vpn-lock.service" ]; then
    "$bin" lock-uninstall < /dev/null > /dev/null || exit 1
fi
if [ -f "$root/etc/systemd/system/wave-vpn-service.socket" ]; then
    "$bin" service-uninstall || exit 1
fi
rmdir "$root/etc/wave-vpn" 2> /dev/null || true
echo 'Wave VPN: disconnected, turned off Network Lock and removed the networking helper.'
echo 'Your settings in ~/.config/wave-vpn are kept, including the Network Lock choice.'
exit 0
