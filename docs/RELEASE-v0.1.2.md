# Wave VPN for Linux 0.1.2

Early release for x86_64 Linux with systemd. Anyone is welcome to use it, but it hasn't been tested on every system yet.

## Download

Download the file for your system and run the command under it in the folder you saved it to. Then open **Wave VPN** from your app menu.

**Ubuntu, Debian, Mint:** [wave-vpn_0.1.2-1_amd64.deb](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn_0.1.2-1_amd64.deb) (14 MB)

```sh
sudo apt install ./wave-vpn_0.1.2-1_amd64.deb
```

**Fedora:** [wave-vpn-0.1.2-1.x86_64.rpm](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-1.x86_64.rpm) (15 MB)

```sh
sudo dnf install ./wave-vpn-0.1.2-1.x86_64.rpm
```

**openSUSE:** the same [.rpm file](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-1.x86_64.rpm)

```sh
sudo zypper install --allow-unsigned-rpm ./wave-vpn-0.1.2-1.x86_64.rpm
```

**Arch, CachyOS, Manjaro:** [wave-vpn-0.1.2-1-x86_64.pkg.tar.zst](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-1-x86_64.pkg.tar.zst) (14 MB)

```sh
sudo pacman -U ./wave-vpn-0.1.2-1-x86_64.pkg.tar.zst
```

**Other distributions:** [wave-vpn-0.1.2-x86_64-unknown-linux-gnu.tar.gz](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-x86_64-unknown-linux-gnu.tar.gz) (14 MB)

```sh
tar xf wave-vpn-0.1.2-x86_64-unknown-linux-gnu.tar.gz
cd wave-vpn-0.1.2-x86_64-unknown-linux-gnu
./install.sh
```

The packages also install WireGuard tools, nftables, iproute2 and libsecret. `install.sh` offers to install them.

## What's new

- Packages for Debian, Ubuntu, Fedora, openSUSE and Arch.
- The tarball now includes a `README.txt` and an `install.sh` that sets everything up.
- Uninstalling disconnects and turns off Network Lock first.
- **Network Lock is now off by default.** Turn it on in **Settings → Network lock**. If you already chose a setting, it's kept.

**Upgrading from 0.1.0 or 0.1.1?** Disconnect and quit Wave VPN first. Before you switch to a package, remove the old install with `sudo bash scripts/install.sh uninstall` from its source folder. See the [user guide](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/docs/USER-GUIDE.md#upgrading-and-uninstalling).

<details>
<summary><b>Verify your download</b> (optional)</summary>

Download [SHA256SUMS](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-x86_64-unknown-linux-gnu-SHA256SUMS) and [SHA256SUMS.asc](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-x86_64-unknown-linux-gnu-SHA256SUMS.asc) into the same folder, then run:

```sh
gpg --recv-keys 414018C0BBB8C0A70A80BF570AA32CCB2C0B3951
gpg --verify wave-vpn-0.1.2-x86_64-unknown-linux-gnu-SHA256SUMS.asc
sha256sum --ignore-missing -c wave-vpn-0.1.2-x86_64-unknown-linux-gnu-SHA256SUMS
```

The key fingerprint must be `4140 18C0 BBB8 C0A7 0A80  BF57 0AA3 2CCB 2C0B 3951`. Check it against [SECURITY.md](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/SECURITY.md) or [vpn.vwave.me](https://vpn.vwave.me/#verify), not against this page. The release workflow built and signed these files from the `v0.1.2` tag. It used nFPM 2.48.0 for the packages.

</details>

[Source code](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-source.tar.gz) · [Changelog](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/CHANGELOG.md) · [User guide](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/docs/USER-GUIDE.md) · [Report a problem](https://github.com/VesselWave/wave-vpn-desktop/issues)
