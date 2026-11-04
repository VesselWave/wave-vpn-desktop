# Wave VPN for Linux 0.1.1

**Early release** · Ubuntu 24.04 x86_64 · GPL-3.0-or-later

**v0.1.1 is an early release.** Anyone is welcome to use it, but it hasn't been fully tested on every device and system yet.

## What's new

- **New look**, shared with the other Wave apps and websites: a blue-black dark theme, pill-shaped buttons, segments and filter chips, and one consistent set of corner sizes.

Only the interface changed. Networking, Network Lock, the root helper and recovery are the same as in 0.1.0.

## Install or update

```sh
sudo apt install wireguard-tools iproute2 nftables libsecret-tools
tar -xzf wave-vpn-0.1.1-source.tar.gz
tar -xzf wave-vpn-0.1.1-x86_64-unknown-linux-gnu.tar.gz
sudo bash wave-vpn-0.1.1/scripts/install.sh install \
  "$PWD/wave-vpn-0.1.1-x86_64-unknown-linux-gnu/usr/local/bin"
wave-vpn-gui
```

To upgrade from 0.1.0, disconnect and quit Wave VPN first, then run the same commands. Your preferences are kept. Keep the source folder: it holds the uninstaller. More in the [user guide](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.1/docs/USER-GUIDE.md).

## Downloads

| File | What it is |
| --- | --- |
| `wave-vpn-0.1.1-x86_64-unknown-linux-gnu.tar.gz` | App, helper and service files, with notices and build records |
| `wave-vpn-0.1.1-source.tar.gz` | Exact tagged source, including the installer |
| `…-SHA256SUMS`, `…-SHA256SUMS.asc`, `…-signing-key.asc` | Checksums, their OpenPGP signature and the public key |

## Verify

```text
4140 18C0 BBB8 C0A7 0A80  BF57 0AA3 2CCB 2C0B 3951
```

```sh
gpg --recv-keys 414018C0BBB8C0A70A80BF570AA32CCB2C0B3951
gpg --verify wave-vpn-0.1.1-x86_64-unknown-linux-gnu-SHA256SUMS.asc \
             wave-vpn-0.1.1-x86_64-unknown-linux-gnu-SHA256SUMS
sha256sum --ignore-missing -c wave-vpn-0.1.1-x86_64-unknown-linux-gnu-SHA256SUMS
```

Compare the fingerprint with [SECURITY.md](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.1/SECURITY.md) or [vpn.vwave.me](https://vpn.vwave.me/#verify), not with this page. Built and signed by the [release workflow](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.1/.github/workflows/release-build.yml) from the `v0.1.1` tag.

Earlier notes: [0.1.0](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.1/docs/RELEASE-v0.1.0.md) · [changelog](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.1/CHANGELOG.md)
