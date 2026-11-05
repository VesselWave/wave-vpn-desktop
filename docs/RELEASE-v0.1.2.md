# Wave VPN for Linux 0.1.2

**Early release** · x86_64 Linux with systemd · GPL-3.0-or-later

**v0.1.2 is an early release.** Anyone is welcome to use it, but it hasn't been fully tested on every device and system yet.

## What's new

- **Packages for your distribution.** Download one file and install it with your package manager, which also installs WireGuard tools, nftables, iproute2 and libsecret.
- **A tarball that explains itself.** It now holds a `README.txt` and an `install.sh` that asks for your password, offers to install missing tools, and sets everything up. You no longer need the source archive to install.
- **Clean uninstall.** Removing the package, or `sudo wave-vpn-uninstall` for the tarball, disconnects and turns off Network Lock before removing the app.

Networking, Network Lock, the root helper and recovery are unchanged from 0.1.1.

## Install

| System | Download | Install with |
| --- | --- | --- |
| Ubuntu, Debian, Mint | `wave-vpn_0.1.2-1_amd64.deb` | `sudo apt install ./wave-vpn_0.1.2-1_amd64.deb` |
| Fedora, openSUSE | `wave-vpn-0.1.2-1.x86_64.rpm` | `sudo dnf install ./wave-vpn-0.1.2-1.x86_64.rpm` |
| Arch, CachyOS, Manjaro | `wave-vpn-0.1.2-1-x86_64.pkg.tar.zst` | `sudo pacman -U wave-vpn-0.1.2-1-x86_64.pkg.tar.zst` |
| Anything else | `wave-vpn-0.1.2-x86_64-unknown-linux-gnu.tar.gz` | Extract, then `./install.sh` in that folder |

Then open **Wave VPN** from your app menu.

**Upgrading from 0.1.0 or 0.1.1:** disconnect and quit Wave VPN. To keep using `/usr/local`, run `./install.sh` from the new tarball. To switch to a package, first remove the old install with `sudo bash wave-vpn-0.1.1/scripts/install.sh uninstall` from its source folder. Preferences are kept either way. More in the [user guide](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/docs/USER-GUIDE.md).

## Downloads

| File | What it is |
| --- | --- |
| `wave-vpn_0.1.2-1_amd64.deb`, `wave-vpn-0.1.2-1.x86_64.rpm`, `wave-vpn-0.1.2-1-x86_64.pkg.tar.zst` | The app for Debian-, RPM- and Arch-based systems |
| `wave-vpn-0.1.2-x86_64-unknown-linux-gnu.tar.gz` | The app with `install.sh`, for any x86_64 distribution with systemd |
| `wave-vpn-0.1.2-source.tar.gz` | Exact tagged source |
| `…-SHA256SUMS`, `…-SHA256SUMS.asc`, `…-signing-key.asc` | Checksums for every file above, their OpenPGP signature and the public key |

## Verify

```text
4140 18C0 BBB8 C0A7 0A80  BF57 0AA3 2CCB 2C0B 3951
```

```sh
gpg --recv-keys 414018C0BBB8C0A70A80BF570AA32CCB2C0B3951
gpg --verify wave-vpn-0.1.2-x86_64-unknown-linux-gnu-SHA256SUMS.asc \
             wave-vpn-0.1.2-x86_64-unknown-linux-gnu-SHA256SUMS
sha256sum --ignore-missing -c wave-vpn-0.1.2-x86_64-unknown-linux-gnu-SHA256SUMS
```

Compare the fingerprint with [SECURITY.md](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/SECURITY.md) or [vpn.vwave.me](https://vpn.vwave.me/#verify), not with this page. Built and signed by the [release workflow](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/.github/workflows/release-build.yml) from the `v0.1.2` tag; packages are built with nFPM 2.48.0.

Earlier notes: [0.1.1](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/docs/RELEASE-v0.1.1.md) · [changelog](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/CHANGELOG.md)
