Wave VPN @VERSION@ for Linux
========================

Unofficial, open-source WireGuard client for AirVPN.


INSTALL
-------

1. Open a terminal in this folder.
   (Right-click an empty spot in the folder and choose "Open in Terminal".)

2. Run:

       ./install.sh

   It asks for your password, offers to install the system tools Wave VPN
   needs (WireGuard tools, iproute2, nftables, libsecret), and sets up the
   networking helper so connecting never asks for a password again.

3. Open "Wave VPN" from your app menu and sign in with your AirVPN account.

You can delete this folder afterwards.


UNINSTALL
---------

       sudo wave-vpn-uninstall

This disconnects, turns off Network Lock and removes Wave VPN.
Your settings in ~/.config/wave-vpn are kept.


PACKAGES
--------

Ubuntu/Debian (.deb), Fedora/openSUSE (.rpm) and Arch/CachyOS
(.pkg.tar.zst) packages install the same app with your package manager:
https://vpn.vwave.me/#download


MORE
----

User guide:  https://github.com/VesselWave/wave-vpn-desktop/blob/v@VERSION@/docs/USER-GUIDE.md
Help:        https://github.com/VesselWave/wave-vpn-desktop/issues
License:     GPL-3.0-or-later, see LICENSE. Notices and build records are in
             files/usr/local/share/doc/wave-vpn/.
