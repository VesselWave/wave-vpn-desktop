# Changelog

## 0.1.2 · 2026-11-05 · early release

Easier installation. [Release notes](docs/RELEASE-v0.1.2.md).

**Added**

- Native packages: `.deb` for Ubuntu and Debian, `.rpm` for Fedora and openSUSE, `.pkg.tar.zst` for Arch and CachyOS, each pulling in its dependencies
- `sudo wave-vpn-uninstall` for tarball installs; no need to keep the download folder

**Changed**

- The tarball is self-contained: `README.txt`, `install.sh` and `LICENSE` on top, the installed files in `files/`. No separate source download needed to install
- `./install.sh` asks for your password itself, names missing dependencies for apt, dnf, pacman or zypper and offers to install them
- Uninstalling disconnects and turns off Network Lock first instead of refusing
- Connecting from an uninstalled copy says Wave VPN isn't installed yet, instead of failing helper setup
- Installed documentation keeps the `docs/` folder, so the README and `docs/README.md` no longer overwrite each other

## 0.1.1 · 2026-11-03 · early release

New look; interface only. [Release notes](docs/RELEASE-v0.1.1.md).

**Changed**

- VesselWave family look shared with Wave Remote and both websites: blue-black dark theme, pill buttons, segments and filter chips, one corner scale

## 0.1.0 · 2026-11-02 · early release

First public build. [Release notes](docs/RELEASE-v0.1.0.md).

**Added**

- WireGuard client for AirVPN, written in Rust with Iced: sign-in, servers, Network Lock, status, logs and tray
- Secret Service account storage and read-only Eddie profile import
- Root-owned socket helper, firewall-state reconciliation, crash recovery, cancellable setup and retry backoff
- AirVPN IPv4, IPv6 and DNS checks; reviewed support report you can preview and export
- Source and binary release packaging with checksum and signature verification

**Status**

Authenticated and isolated network tests pass in the environments in [verification](docs/VERIFICATION.md).

**Not planned for now:** boot autoconnect, an advanced networking UI, other protocols and other desktop VPN backends.
