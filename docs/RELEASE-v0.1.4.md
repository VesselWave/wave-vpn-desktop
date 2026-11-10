More reliable DNS, upgrades and recovery, with clearer connection status across Linux desktops.

Early release · Linux x86_64 (Intel / AMD, 64-bit) · systemd required. Development packages exercised on Ubuntu 24.04/26.04, Debian 13, Fedora 45, openSUSE Tumbleweed, Arch KDE and Mint 22.3; release qualification and physical suspend testing remain incomplete.

## What's new

- Connect on Ubuntu 26.04 with its stock AppArmor policy; keep tunnel DNS in place on Debian and openSUSE when NetworkManager refreshes the network.
- See **Blocking traffic** when Network Lock remains on, with an action to turn it off. Wave reports and restores lock rules removed by another program.
- Upgrade without needrestart dropping an active tunnel; recover leftover networking after a worker crash.
- Restore accounts from KWallet on openSUSE, and retry a locked keyring instead of appearing signed out.
- Cancel sign-in or stop after its 45-second limit; get clearer helper and connection errors.
- Run one window per user, use less CPU while idle, and see country names and accurate server-switch actions.

> [!IMPORTANT]
> **Upgrading from published 0.1.3:** disconnect and quit before installing, then reboot to ensure the new networking helper is running. The old helper can remain active until reboot.
>
> Network Lock remains **off by default** on new installs. With it off, container DNS and NetworkManager connectivity checks can use the physical network, and DNS can briefly do so during switching. Enable Network Lock to block this traffic; containers and NAT virtual machines then have no network. See the [user guide](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.4/docs/USER-GUIDE.md#known-limits-from-distribution-testing).

Best server remains a latency/load heuristic; throughput and long-term reliability are unverified.

**Upgrade the installed helper together with the app.** Disconnect and quit first, then install the updated package or rerun the tarball installer. Bootstrap exceptions are bounded public endpoints, not cryptographically verified provider addresses; see [Network Lock's trust boundary](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.4/SECURITY.md).

> [!IMPORTANT]
> Network Lock is **off by default** on new installs. Enable it in **Settings → Network Lock** to block traffic outside the VPN, including while disconnected. Your existing choice is preserved when upgrading.

## Downloads

| Distribution | Download |
| --- | --- |
| Ubuntu, Debian, Linux Mint | [**.deb**](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.4/wave-vpn_0.1.4-1_amd64.deb) |
| Fedora, openSUSE | [**.rpm**](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.4/wave-vpn-0.1.4-1.x86_64.rpm) |
| Arch, CachyOS, Manjaro | [**.pkg.tar.zst**](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.4/wave-vpn-0.1.4-1-x86_64.pkg.tar.zst) |
| Other distributions with systemd | [**.tar.gz**](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.4/wave-vpn-0.1.4-x86_64-unknown-linux-gnu.tar.gz) |

**Upgrading from 0.1.0 or 0.1.1:** disconnect and quit Wave VPN first. Before switching to a package, remove the old install with `sudo bash scripts/install.sh uninstall` from its source folder. [Upgrade guide →](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.4/docs/USER-GUIDE.md#upgrading-and-uninstalling)

<details>
<summary>Installation commands</summary>

Run the command for your distribution in the folder containing your download, then open **Wave VPN** from your app menu.

**Ubuntu / Debian / Mint**

```sh
sudo apt install ./wave-vpn_0.1.4-1_amd64.deb
```

**Fedora**

```sh
sudo dnf install ./wave-vpn-0.1.4-1.x86_64.rpm
```

**openSUSE**

```sh
sudo zypper install --allow-unsigned-rpm ./wave-vpn-0.1.4-1.x86_64.rpm
```

**Arch / CachyOS / Manjaro**

```sh
sudo pacman -U ./wave-vpn-0.1.4-1-x86_64.pkg.tar.zst
```

**Other distributions**

```sh
tar xf wave-vpn-0.1.4-x86_64-unknown-linux-gnu.tar.gz
cd wave-vpn-0.1.4-x86_64-unknown-linux-gnu
./install.sh
```

Packages install WireGuard tools, nftables, iproute2 and libsecret. The tarball installer offers to install missing dependencies.

</details>

<details>
<summary>Verify checksums and signature</summary>

Download [SHA256SUMS](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.4/wave-vpn-0.1.4-x86_64-unknown-linux-gnu-SHA256SUMS) and [SHA256SUMS.asc](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.4/wave-vpn-0.1.4-x86_64-unknown-linux-gnu-SHA256SUMS.asc) into the same folder, then run:

```sh
gpg --recv-keys 414018C0BBB8C0A70A80BF570AA32CCB2C0B3951
gpg --verify wave-vpn-0.1.4-x86_64-unknown-linux-gnu-SHA256SUMS.asc
sha256sum --ignore-missing -c wave-vpn-0.1.4-x86_64-unknown-linux-gnu-SHA256SUMS
```

The key fingerprint must be `4140 18C0 BBB8 C0A7 0A80  BF57 0AA3 2CCB 2C0B 3951`. Confirm it independently against [SECURITY.md](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.4/SECURITY.md) or [vpn.vwave.me](https://vpn.vwave.me/#verify).

</details>

---

[User guide](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.4/docs/USER-GUIDE.md) · [Full changelog](https://github.com/VesselWave/wave-vpn-desktop/blob/v0.1.4/CHANGELOG.md) · [Source archive](https://github.com/VesselWave/wave-vpn-desktop/releases/download/v0.1.4/wave-vpn-0.1.4-source.tar.gz) · [Report an issue](https://github.com/VesselWave/wave-vpn-desktop/issues)
