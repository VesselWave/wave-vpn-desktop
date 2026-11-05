# Installation and support

**Wave VPN is in early release.** Anyone is welcome to use it, but it hasn't been fully tested on every device and system yet.

## Tested systems

| System | Evidence |
| --- | --- |
| Ubuntu 24.04 x86_64, GNOME Wayland | Networking and desktop checks; helper security; EFI boot with the boot lock; 0.1.2 `.deb` and tarball install, upgrade and removal |
| Ubuntu 24.04 ARM64, GNOME X11 | Networking and desktop checks; authenticated live suite from the v0.1.0 source |
| CachyOS x86_64, physical laptop, niri Wayland | Shipped binary: GUI rendering, staged install and uninstall, all 19 isolated network scenarios; 0.1.2 Arch package and tarball install and removal |
| Fedora 45, openSUSE Tumbleweed (containers) | 0.1.2 `.rpm` dependency resolution, install and removal only; no desktop session or networking |
| Debian ARM64 Xfce, Fedora ARM64 KDE | Isolated network suite only, not full GUI support |

Other distributions and architectures may work but haven't been tested. Published binaries and packages are x86_64 only. macOS builds the UI without VPN connections. Windows, OpenVPN and AirVPN account-side port forwarding and DDNS aren't supported. Exact binaries and results: [verification](VERIFICATION.md).

## Requirements

- Kernel WireGuard, `wireguard-tools`, `iproute2`, `nftables`, `getent` and systemd. The .deb, .rpm and Arch packages depend on these; the tarball installer offers to install missing ones with apt, dnf, pacman or zypper.
- `secret-tool` (libsecret) and an unlocked desktop keyring for saved accounts and Eddie keyring import.
- DNS through systemd-resolved, resolvconf or a plain `/etc/resolv.conf`; Wave VPN detects which.
- `pkexec` with a graphical polkit agent, if the networking helper is set up from the app on the first Connect rather than by a `sudo` install.

## Installing

Pick the package for your system in the [README](../README.md#install): `.deb` for Ubuntu and Debian, `.rpm` for Fedora and openSUSE, `.pkg.tar.zst` for Arch and CachyOS, or the tarball with `./install.sh` for anything else. Packages install to `/usr`, the tarball to `/usr/local`; use one or the other. Rust isn't needed.

Wave VPN then sets up a root-owned, socket-activated networking helper and authorizes your desktop user: during a `sudo` install from a terminal, or on your first Connect after a password prompt. After that, connecting and Network Lock never ask for a password, including after reboot. The helper doesn't enable a boot lock or autoconnect and never stores credentials.

Running `wave-vpn-gui` straight from the extracted tarball works for browsing, but connecting needs an installed copy: the helper only runs a root-owned binary.

Upgrading and uninstalling are in the [user guide](USER-GUIDE.md#upgrading-and-uninstalling).

## Troubleshooting

**No internet after disconnecting or quitting.** Network Lock is on: once you enable it, it blocks everything outside the VPN until you turn it off. Turn it off in Settings, or run `sudo wave-vpn lock-disable`.

**Sign-in fails after a broken VPN session.** A wrong password and an unreachable AirVPN show different messages, so a network failure doesn't mean your password is wrong. Use **Overview → Recover** (it works while signed out), then retry. An enabled Network Lock is kept; turn it off separately only if you mean to. A new connection also repairs Wave's leftover state after a crash.

**Protection status shows unknown.** Wave VPN reads the firewall through the helper. A missing helper, a failed query or another program's table gives *unknown*. A stale cached file is never taken as proof either way. Reopening the app shows whether recovery is needed and whether a worker is still running.

**Another VPN left DNS or firewall state behind.** Fix it in that VPN. Don't flush the system firewall or delete recovery records as a general fix.

## Reporting a problem

Open bugs as [GitHub issues](https://github.com/VesselWave/wave-vpn-desktop/issues). Ask questions in the [AirVPN forum thread](https://airvpn.org/forums/topic/82017-wave-vpn-open-source-wireguard-client-for-android-and-linux-unofficial-v011/).

Security issues go privately to **vesselwave@protonmail.com** ([SECURITY.md](../SECURITY.md)).

For everything else, attach a support report: **Logs → Support report** in the app, or from a terminal:

```sh
wave-vpn diagnostics                                   # preview
wave-vpn diagnostics --output wave-diagnostics.json    # save
```

It contains check and protection results, tool availability and fixed failure categories, with no account names, secrets, addresses, preferences or raw logs. Export writes a new file (mode 0600) and never overwrites an existing one. Also tell us:

- what you did and what you saw
- distribution, kernel and Wave VPN version
- whether Recover cleaned up, and whether turning off Network Lock restored internet

Don't attach private profiles, keyring contents or raw logs instead.
