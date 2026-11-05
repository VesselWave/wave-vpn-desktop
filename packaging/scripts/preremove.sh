#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Runs before the Wave VPN package is removed. Upgrades keep everything.
# Arguments: Debian "remove" or "upgrade"; RPM 0 for removal, 1 or more for
# upgrade; Arch calls this only for removal, with the package version.
case "${1:-}" in
    upgrade | failed-upgrade | deconfigure | [1-9] | [1-9][0-9]) exit 0 ;;
esac
bin=/usr/bin/wave-vpn
[ -x "$bin" ] || exit 0
# Nothing to undo unless Wave has set something up since boot or installed units.
state=
for path in /run/wave-vpn /run/wave-vpn-protection.json /etc/systemd/system/wave-vpn-service.socket \
    /etc/systemd/system/wave-vpn-lock.service; do
    if [ -e "$path" ]; then state=1; fi
done
[ -n "$state" ] || exit 0
# Never leave a connection or Network Lock behind without the app to undo it.
systemctl stop wave-vpn-service.socket wave-vpn-service.service 2> /dev/null || true
if ! "$bin" lock-disable < /dev/null > /dev/null; then
    echo 'Wave VPN: could not disconnect or turn off Network Lock; run sudo wave-vpn recover.' >&2
    exit 1
fi
if [ -f /etc/systemd/system/wave-vpn-lock.service ]; then
    "$bin" lock-uninstall < /dev/null > /dev/null || exit 1
fi
if [ -f /etc/systemd/system/wave-vpn-service.socket ]; then
    "$bin" service-uninstall || exit 1
fi
rmdir /etc/wave-vpn 2> /dev/null || true
echo 'Wave VPN: disconnected, turned off Network Lock and removed the networking helper.'
exit 0
