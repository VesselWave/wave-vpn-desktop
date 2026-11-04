<p align="center">
  <img src="docs/brand/cover.png" alt="Wave VPN: an open-source WireGuard client for AirVPN on Android and Linux." width="100%">
</p>

<p align="center">
  <a href="https://github.com/VesselWave/wave-vpn-desktop/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/VesselWave/wave-vpn-desktop?include_prereleases&amp;style=flat-square&amp;color=2f6bff"></a>
  <a href="LICENSE"><img alt="License: GPL-3.0-or-later" src="https://img.shields.io/badge/license-GPL--3.0--or--later-2f6bff?style=flat-square"></a>
  <img alt="Linux x86_64" src="https://img.shields.io/badge/Linux-x86__64-2f6bff?style=flat-square">
</p>

An unofficial WireGuard client for [AirVPN](https://airvpn.org) on the Linux desktop. Sign in with your AirVPN account, pick a server and connect, with a Network Lock firewall that keeps traffic inside the tunnel. Open source, written in Rust, with signed releases.

**[Download from vpn.vwave.me](https://vpn.vwave.me)** · [Release notes](docs/RELEASE-v0.1.1.md) · [Android app](https://github.com/VesselWave/wave-vpn-mobile)

<p align="center">
  <img src="docs/screenshots/overview.png" alt="Overview: connected to Amsterdam, with IPv4, IPv6 and DNS verified, live rates and session totals" width="49%">
  <img src="docs/screenshots/servers.png" alt="Servers: a sortable table with location, load, users, ping and score, plus a Best server choice" width="49%">
</p>

- Sign in with your AirVPN account; credentials go to your desktop keyring
- Best-server choice, or a sortable server table with search, favorites and countries
- **Network Lock**: an nftables firewall that blocks traffic outside the tunnel, and stays on after Disconnect, Quit or a crash until you turn it off
- Checks every connection: AirVPN confirms the tunnel's IPv4 and IPv6 identity and that your DNS reaches AirVPN
- **Recover** undoes Wave's own DNS and tunnel changes after a failure, even before you sign in
- Tray indicator, connection log and a reviewed support report with nothing private in it
- Imports your existing Eddie account (read-only), and exports WireGuard profiles for other tools

> [!NOTE]
> **Wave VPN is in early release.** Anyone is welcome to use it, but it hasn't been fully tested on every device and system yet.

## Install

Built for Ubuntu 24.04 (x86_64, GNOME, systemd). Other distributions may work but aren't tested.

```sh
sudo apt install wireguard-tools iproute2 nftables libsecret-tools
```

Download the binary and source archives, checksums, signature and key from [vpn.vwave.me](https://vpn.vwave.me), [verify them](#verify-your-download), then:

```sh
tar -xzf wave-vpn-0.1.1-source.tar.gz
tar -xzf wave-vpn-0.1.1-x86_64-unknown-linux-gnu.tar.gz
sudo bash wave-vpn-0.1.1/scripts/install.sh install \
  "$PWD/wave-vpn-0.1.1-x86_64-unknown-linux-gnu/usr/local/bin"
wave-vpn-gui
```

The installer adds a small root-owned helper and authorizes your user once, so connecting never asks for a password. Keep the source folder: it holds the uninstaller. Upgrading, uninstalling and recovery are in the [user guide](docs/USER-GUIDE.md).

## Verify your download

Every release ships signed SHA-256 checksums. The release key is on [keys.openpgp.org](https://keys.openpgp.org/search?q=414018C0BBB8C0A70A80BF570AA32CCB2C0B3951) and [vpn.vwave.me](https://vpn.vwave.me/#verify):

```text
4140 18C0 BBB8 C0A7 0A80  BF57 0AA3 2CCB 2C0B 3951
```

```sh
gpg --recv-keys 414018C0BBB8C0A70A80BF570AA32CCB2C0B3951
gpg --verify wave-vpn-0.1.1-x86_64-unknown-linux-gnu-SHA256SUMS.asc \
             wave-vpn-0.1.1-x86_64-unknown-linux-gnu-SHA256SUMS
sha256sum --ignore-missing -c wave-vpn-0.1.1-x86_64-unknown-linux-gnu-SHA256SUMS
```

If the fingerprint differs from this one, from [SECURITY.md](SECURITY.md) and from the website, don't install; report it privately.

## How it stays safe

The app runs as your user. Networking goes through a socket-activated helper that accepts only a fixed set of actions from the user it authorized, never shell commands or raw config. It sets up its own WireGuard interface, routes and nftables table, and keeps a root-only recovery record so a crash can always be undone. Wave VPN talks only to AirVPN: no analytics, no telemetry, and support reports are never uploaded. Details in the [user guide](docs/USER-GUIDE.md) and [SECURITY.md](SECURITY.md).

## Building

Needs a recent stable Rust. VPN connections need Linux; the UI also builds on macOS.

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
sudo bash scripts/install.sh install
```

More, including the isolated network test suite, in [CONTRIBUTING.md](CONTRIBUTING.md); test evidence is indexed in [docs/](docs/README.md).

## How this was made

Most of Wave VPN's code, tests and documentation were written by AI coding agents, **Claude Opus 5.5** and **GPT 6.1 Sol**, working under the maintainer's direction. The maintainer sets the scope, makes the product and security decisions, provides the test devices, lab machines and AirVPN account, holds the signing keys, and decides what gets merged and released.

That's why everything is published: the source, the protocol references in [provenance](docs/PROVENANCE.md), and every [test report](docs/README.md), including the failures. Please judge the app by that evidence, not by who or what typed it, and report anything that looks wrong.

---

GPL-3.0-or-later. Independent project, not affiliated with or endorsed by AirVPN. Report security issues privately to **vesselwave@protonmail.com** ([security policy](SECURITY.md)).

## Published history privacy

The repository history and release packages were sanitized on 2026-11-03.
Lab records use example identities and addresses; private management details
are omitted. Sanitized release archives and fresh signed checksum manifests are
available. The APK and executable bytes are unchanged. Download the current
archives, checksums and signatures together. The GitHub repositories were
subsequently recreated under the same names without the old PR threads. Existing
clones may retain old history.
