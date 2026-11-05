Native Linux packages make Wave VPN easier to install and update. This release also makes Network Lock opt-in and simplifies uninstalling.

Early release · Linux x86_64 (Intel / AMD, 64-bit) · systemd required. Tested most on Ubuntu 24.04 GNOME; testing on other systems is still in progress.

## What's new

- **Native packages** for Debian / Ubuntu, Fedora / openSUSE, and Arch-based distributions, with dependencies installed by your package manager.
- **Self-contained tarball** with an installer and README. No separate source download needed; remove it later with `sudo wave-vpn-uninstall`.
- **Safer uninstall:** disconnects the VPN and turns off Network Lock before removing the app.
- **Clearer setup errors** when connecting from a copy that hasn't been installed yet.

> [!IMPORTANT]
> Network Lock is **off by default** on new installs. Enable it in **Settings → Network lock** to block traffic outside the VPN, including while disconnected. Your existing choice is preserved when upgrading.

## Downloads

| Distribution | Download |
| --- | --- |
| Ubuntu, Debian, Linux Mint | [**.deb** · 14 MB](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn_0.1.2-1_amd64.deb) |
| Fedora, openSUSE | [**.rpm** · 15 MB](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-1.x86_64.rpm) |
| Arch, CachyOS, Manjaro | [**.pkg.tar.zst** · 14 MB](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-1-x86_64.pkg.tar.zst) |
| Other distributions with systemd | [**.tar.gz** · 14 MB](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-x86_64-unknown-linux-gnu.tar.gz) |

**Upgrading from 0.1.0 or 0.1.1:** disconnect and quit Wave VPN first. Before switching to a package, remove the old install with `sudo bash scripts/install.sh uninstall` from its source folder. [Upgrade guide →](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/docs/USER-GUIDE.md#upgrading-and-uninstalling)

<details>
<summary>Installation commands</summary>

Run the command for your distribution in the folder containing your download, then open **Wave VPN** from your app menu.

**Ubuntu / Debian / Mint**

```sh
sudo apt install ./wave-vpn_0.1.2-1_amd64.deb
```

**Fedora**

```sh
sudo dnf install ./wave-vpn-0.1.2-1.x86_64.rpm
```

**openSUSE**

```sh
sudo zypper install --allow-unsigned-rpm ./wave-vpn-0.1.2-1.x86_64.rpm
```

**Arch / CachyOS / Manjaro**

```sh
sudo pacman -U ./wave-vpn-0.1.2-1-x86_64.pkg.tar.zst
```

**Other distributions**

```sh
tar xf wave-vpn-0.1.2-x86_64-unknown-linux-gnu.tar.gz
cd wave-vpn-0.1.2-x86_64-unknown-linux-gnu
./install.sh
```

Packages install WireGuard tools, nftables, iproute2 and libsecret. The tarball installer offers to install missing dependencies.

</details>

<details>
<summary>Verify checksums and signature</summary>

Download [SHA256SUMS](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-x86_64-unknown-linux-gnu-SHA256SUMS) and [SHA256SUMS.asc](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-x86_64-unknown-linux-gnu-SHA256SUMS.asc) into the same folder, then run:

```sh
gpg --recv-keys 414018C0BBB8C0A70A80BF570AA32CCB2C0B3951
gpg --verify wave-vpn-0.1.2-x86_64-unknown-linux-gnu-SHA256SUMS.asc
sha256sum --ignore-missing -c wave-vpn-0.1.2-x86_64-unknown-linux-gnu-SHA256SUMS
```

The key fingerprint must be `4140 18C0 BBB8 C0A7 0A80  BF57 0AA3 2CCB 2C0B 3951`. Confirm it independently against [SECURITY.md](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/SECURITY.md) or [vpn.vwave.me](https://vpn.vwave.me/#verify).

</details>

---

[User guide](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/docs/USER-GUIDE.md) · [Full changelog](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.2/CHANGELOG.md) · [Source archive](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.2/wave-vpn-0.1.2-source.tar.gz) · [Report an issue](https://github.com/VesselWave/wave-vpn-desktop/issues)
