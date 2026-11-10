# User guide

How Wave VPN for Linux behaves, and what to do when something goes wrong. For installation see the [README](../README.md); for the full support matrix see [installation and support](SUPPORT.md).

## Signing in

Sign in with your AirVPN username and password. Wave VPN saves them in your desktop keyring (Secret Service), so install `libsecret-tools` and keep the keyring unlocked. If no keyring is available, the app warns you before sign-in that credentials would be stored in plain text in a private local file. **Sign out** removes the saved account. With automatic login, your desktop may ask you to unlock the keyring when Wave starts. If you cancel that prompt, Wave says the keyring is locked and offers **Retry**; your saved account is still there.

Already use Eddie? **Import Eddie profile** on the sign-in screen reads Eddie's profile (XML, v1 and v2, with default, password or keyring protection) and fills in the sign-in form. It never changes Eddie's files or keyring entry. Advanced Eddie settings aren't imported.

## Connecting

Pick **Best server** or any server in **Servers**, then connect. Once the tunnel is up, Wave VPN asks AirVPN's check service to confirm:

- **IPv4 / IPv6**: traffic arrives from the AirVPN server you connected to, for each IP family you have
- **DNS**: a fresh lookup through your system resolver reached AirVPN's DNS

The checks run once, right after the tunnel comes up (with up to two spaced retries), and results show in Overview. A failed check (another identity or resolver seen) stops the connection and stays shown as failed; Wave doesn't reconnect automatically, because reconnecting can't fix it. Use **Recover** and connect again. If the check service itself is down, the result says *unavailable*: that isn't evidence of a leak, so the connection stays up and is shown as not verified. While connected, only the local routing table is rechecked; nothing is sent to AirVPN.

## Server list and latency

- The server list refreshes about every 30 minutes (or AirVPN's advertised interval, if longer), never more often than every 5 minutes; random spread only makes the wait up to 20 % longer. The periodic refresh fetches only the server list (plus your account about once a day); **Refresh** in **Servers** also refreshes your account — use it after adding or removing a device or renewing — and is limited to once a minute. If your selected device was removed, Wave switches to one that exists. If a refresh fails, the current list stays usable and Wave retries after 5, 10, 20… minutes (at most an hour), plus random spread.
- Sign-in addresses and AirVPN's public key come from the latest server list and are saved in `~/.config/wave-vpn/bootstrap.json`; the set built into the app is used on first run and as a fallback. Addresses are tried one at a time with short, growing pauses. When the sign-in addresses change, Wave passes the new address/port pairs to Network Lock (sign-in hostnames are looked up once, while the network works, and only their public addresses are allowed). These bootstrap exceptions apply only to apps running as you, and the Network Lock page shows how many are active. The networking helper checks that they are bounded public addresses, but it can't prove they belong to AirVPN; see [SECURITY.md](../SECURITY.md). If the helper can't apply a new set, Wave keeps the previous one, retries in the background (30 s, doubling up to 15 min) and suggests reinstalling with `sudo bash scripts/install.sh install` when the installed helper looks older than the app.
- Ping is a TCP connection to a server's first IPv4 entry address, at most four at a time. Each run probes at most 32 servers: one per location (locations not yet measured first, random order) plus a server you selected explicitly; other servers show the result of their location. Results and the 30-minute wait are kept across restarts in `~/.config/wave-vpn/latency.json` (server names and milliseconds only) and forgotten after a day. Nothing is measured while connecting, connected, reconnecting, changing Network Lock or under Network Lock. A server that doesn't answer shows *Timeout* and ranks after every server that did; servers with no result for themselves or their location rank at the median.

DNS works with systemd-resolved, resolvconf, or a plain `/etc/resolv.conf` (backed up and restored). With a plain `/etc/resolv.conf` (the default on Debian and openSUSE), Wave tells NetworkManager to leave the file alone while connected, using a temporary drop-in under `/run/NetworkManager/conf.d/`. If any other program still rewrites the file, Wave writes the tunnel DNS back within a second and notes it in Logs. On Disconnect or Recover, Wave puts your original file back. If another program wrote a newer file during the session, Wave keeps that file and removes its own backup, so Recover always finishes.

## Network Lock

Network Lock is an nftables firewall that blocks traffic outside the tunnel. It's off until you turn it on in **Settings → Network Lock**, because while it's on you have no internet without the VPN.

- Once on, it stays on after **Disconnect**, **Quit**, **Recover** and crashes, until you turn it off. That's deliberate: a crash must never quietly unblock your traffic.
- You can toggle it while connected; the tunnel keeps running and your choice is remembered.
- If another program removes it (for example `sudo nft flush ruleset`, an `nftables.service` restart or another firewall manager reloading), Wave notices within seconds. While connected, Wave puts its rules back and notes it in Logs. While disconnected, Wave shows a warning and turns the lock back on when that's your saved choice. A lock you didn't choose is only reported. After a reboot the lock is off until you connect; Settings then says your saved choice turns it on again when you connect.
- Allowed while locked: loopback, DHCP and IPv6 neighbour discovery, the tunnel itself, and AirVPN's sign-in addresses (built-in and from the latest server list; TCP to the listed port only). LAN access and ping outside the tunnel are off by default (**Settings → Network Lock**).
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

## Containers and virtual machines

Wave protects the programs on this computer. Containers (Docker, Podman) and NAT virtual machines (libvirt, GNOME Boxes) send their traffic through the host in a way that has two known limits:

- **With Network Lock on, containers and NAT virtual machines have no network, even while connected.** Network Lock blocks all forwarded traffic, and Wave doesn't offer a tunnel exception for it in the app (custom forwarding is [out of scope](PARITY.md)). To use them, turn off Network Lock while they need the network.
- **With Network Lock off, container DNS can bypass the tunnel.** On systems with systemd-resolved (Ubuntu, Fedora), Docker gives containers the full list of upstream DNS servers from `/run/systemd/resolve/resolv.conf`. That list includes your router's DNS next to the tunnel's, and Alpine-based images query all of them at once. Container web traffic still uses the tunnel; only those DNS lookups go to your LAN resolver. Wave keeps the DNS settings of your other network links so that NetworkManager stays in charge of them, which is why the router's server stays on that list. To keep container DNS on the tunnel, start containers with `--dns 10.128.0.1` (AirVPN's resolver, which only answers while connected), or turn on Network Lock, which blocks those lookups (and, as above, the containers' network). Programs on the host itself are not affected.

## Using AirVPN with another WireGuard tool

```sh
wave-vpn profile --login YOUR_USERNAME --server SERVER_NAME --output wave.conf
```

The file is created with mode 0600 and contains your private key. Keep it private.

## Upgrading and uninstalling

**Upgrade** by installing the new release the same way you installed the old one: the new package with your package manager, or `./install.sh` from the new tarball. Preferences are kept and the networking helper is updated too. A package upgrade while connected updates the helper right away and leaves the connection running; the next connection uses the new helper. The tarball installer refuses to run while connected, so disconnect first.

**Uninstall** with the method you installed with:

```sh
sudo apt remove wave-vpn       # Ubuntu, Debian
sudo dnf remove wave-vpn       # Fedora (openSUSE: sudo zypper remove wave-vpn)
sudo pacman -R wave-vpn        # Arch, CachyOS
sudo wave-vpn-uninstall        # tarball install
```

Uninstalling first closes any open Wave VPN window, disconnects, turns off Network Lock and removes the networking helper, its systemd units and the boot lock, so you're never left offline without the app. It leaves dependency packages alone and keeps your preferences in `~/.config/wave-vpn`, even with `apt purge`: the package doesn't touch files in home folders. Those preferences include your Network Lock choice, so if you reinstall, the lock comes back on with your next Connect. To start fresh, delete `~/.config/wave-vpn` (sign out first to remove the saved account from your keyring). Releases before 0.1.2 were uninstalled with `sudo bash wave-vpn-0.1.x/scripts/install.sh uninstall` from their source folder.

## Scope

WireGuard only. OpenVPN, obfuscation, Windows, macOS connections, and AirVPN account-side port forwarding and DDNS are out of scope. Advanced routing, forwarding, DNS and transport tuning are deliberately kept out of the UI; see [scope](PARITY.md).

## Known limits from distribution testing

With Network Lock off, NetworkManager can send its own connectivity check over
the physical network, including plain HTTP on Mint. During a server switch, DNS
can briefly use the normal network resolver. Turn on Network Lock to block
traffic outside the tunnel, including these requests.

When upgrading from the published 0.1.3 package, the old networking helper can
keep running until reboot. Disconnect and reboot after upgrading to use the new
helper. Later builds restart a replaced helper once idle.

On Fedora and openSUSE, reactivating a wired connection with Network Lock on can
make `nmcli con up` report an error; testing observed recovery within 2–40 seconds
without affecting tunnel DNS. An install immediately after boot also stopped a
VM's login screen twice; this has not reproduced in installs after boot completed.
See [the distribution reports](qa/DISTRO-MATRIX-FINAL.md) for coverage and limits.
