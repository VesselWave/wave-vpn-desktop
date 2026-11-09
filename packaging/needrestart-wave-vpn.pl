# SPDX-License-Identifier: GPL-3.0-or-later
# Wave VPN: never let needrestart restart the networking service after an
# upgrade. Its root workers run the active VPN connection, so a restart would
# drop the tunnel. The service restarts itself once idle when its executable
# was replaced, and the next connection uses the upgraded helper.
$nrconf{override_rc}{qr(^wave-vpn-(service|lock)\.service$)} = 0;
