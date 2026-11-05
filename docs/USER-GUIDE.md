# User guide

How Wave VPN for Linux behaves, and what to do when something goes wrong. For installation see the [README](../README.md); for the full support matrix see [installation and support](SUPPORT.md).

## Signing in

Sign in with your AirVPN username and password. Wave VPN saves them in your desktop keyring (Secret Service), so install `libsecret-tools` and keep the keyring unlocked. If no keyring is available, the app warns you before sign-in that credentials would be stored in plain text in a private local file. **Sign out** removes the saved account.

Already use Eddie? **Import Eddie profile** on the sign-in screen reads Eddie's profile (XML, v1 and v2, with default, password or keyring protection) and fills in the sign-in form. It never changes Eddie's files or keyring entry. Advanced Eddie settings aren't imported.

## Connecting

Pick **Best server** or any server in **Servers**, then connect. Once the tunnel is up, Wave VPN asks AirVPN's check service to confirm:

- **IPv4 / IPv6**: traffic arrives from the AirVPN server you connected to, for each IP family you have
- **DNS**: a fresh lookup through your system resolver reached AirVPN's DNS

Results show in Overview and repeat periodically. A failed check stops the connection; auto-reconnect retries with backoff. If the check service itself is down, the result says *unavailable*, which isn't evidence of a leak.

DNS works with systemd-resolved, resolvconf, or a plain `/etc/resolv.conf` (backed up and restored).

## Network Lock

Network Lock is an nftables firewall that blocks traffic outside the tunnel.

- It stays on after **Disconnect**, **Quit**, **Recover** and crashes, until you turn it off. That's deliberate: a crash must never quietly unblock your traffic.
- You can toggle it while connected; the tunnel keeps running and your choice is remembered.
- Allowed while locked: loopback, DHCP and IPv6 neighbour discovery, the tunnel itself, and AirVPN's pinned sign-in addresses. LAN access and ping outside the tunnel are off by default (**Settings → Network lock**).
- To get ordinary internet back, turn it off in **Settings**, or run `sudo wave-vpn lock-disable`.

The lock survives app exit but not a reboot. For a lock that is active from boot, install the optional service: `sudo wave-vpn lock-install`, then `sudo wave-vpn lock-enable`. Boot autoconnect isn't available yet.

## When something goes wrong

**Recover** (Overview, available even before sign-in) undoes Wave's own journaled DNS and tunnel changes and keeps Network Lock as you set it. Sign-in uses pinned AirVPN addresses, so it works even when public DNS is broken. A rejected account and a network failure are reported differently.

If Eddie left its own DNS or firewall state behind, recover that in Eddie first; Wave VPN never resets other apps' networking.

From a terminal:

```sh
sudo wave-vpn recover                          # undo Wave's DNS/tunnel changes, keep the lock
sudo wave-vpn lock-disable                     # restore ordinary internet access
wave-vpn service-client network-status         # show firewall and recovery state, change nothing
wave-vpn diagnostics                           # metadata snapshot for a bug report
```

**Logs → Support report** shows exactly what would be shared: check and protection results, tool availability and fixed failure categories. Accounts, keys, addresses and raw logs are left out, and nothing is uploaded; you copy or save it yourself.

## Using AirVPN with another WireGuard tool

```sh
wave-vpn profile --login YOUR_USERNAME --server SERVER_NAME --output wave.conf
```

The file is created with mode 0600 and contains your private key. Keep it private.

## Upgrading and uninstalling

**Upgrade** by installing the new release the same way you installed the old one: the new package with your package manager, or `./install.sh` from the new tarball. Preferences are kept and the networking helper is updated too. Disconnect first: the tarball installer refuses to run while connected, and a package installed while connected asks you to run `sudo wave-vpn service-install` once you disconnect.

**Uninstall** with the method you installed with:

```sh
sudo apt remove wave-vpn       # Ubuntu, Debian
sudo dnf remove wave-vpn       # Fedora (openSUSE: sudo zypper remove wave-vpn)
sudo pacman -R wave-vpn        # Arch, CachyOS
sudo wave-vpn-uninstall        # tarball install
```

Uninstalling first disconnects, turns off Network Lock and removes the networking helper, its systemd units and the boot lock, so you're never left offline without the app. It keeps your preferences in `~/.config/wave-vpn` and leaves dependency packages alone. Releases before 0.1.2 were uninstalled with `sudo bash wave-vpn-0.1.x/scripts/install.sh uninstall` from their source folder.

## Scope

WireGuard only. OpenVPN, obfuscation, Windows, macOS connections, and AirVPN account-side port forwarding and DDNS are out of scope. Advanced routing, forwarding, DNS and transport tuning are deliberately kept out of the UI; see [scope](PARITY.md).
