# User guide

How Wave VPN for Linux behaves, and what to do when something goes wrong. For installation see the [README](../README.md); for the full support matrix see [installation and support](SUPPORT.md).

## Signing in

Sign in with your AirVPN username and password. Wave VPN saves them in your desktop keyring (Secret Service), so install `libsecret-tools` and keep the keyring unlocked. If no keyring is available, the app warns you before sign-in that credentials would be stored in plain text in a private local file. **Sign out** removes the saved account.

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

DNS works with systemd-resolved, resolvconf, or a plain `/etc/resolv.conf` (backed up and restored).

## Network Lock

Network Lock is an nftables firewall that blocks traffic outside the tunnel.

- It stays on after **Disconnect**, **Quit**, **Recover** and crashes, until you turn it off. That's deliberate: a crash must never quietly unblock your traffic.
- You can toggle it while connected; the tunnel keeps running and your choice is remembered.
- Allowed while locked: loopback, DHCP and IPv6 neighbour discovery, the tunnel itself, and AirVPN's sign-in addresses (built-in and from the latest server list; TCP to the listed port only). LAN access and ping outside the tunnel are off by default (**Settings → Network lock**).
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

Disconnect and quit Wave VPN, then run the same install command with the new release. Preferences are kept, and installation refuses to run while a recovery record is active.

```sh
sudo wave-vpn lock-uninstall                   # only if you installed the boot lock
sudo bash wave-vpn-0.1.1/scripts/install.sh uninstall
```

The uninstaller removes Wave's binaries, helper and systemd units. It keeps your preferences in `~/.config/wave-vpn`, leaves dependency packages alone, and refuses to run while a recovery record or boot lock is installed.

## Scope

WireGuard only. OpenVPN, obfuscation, Windows, macOS connections, and AirVPN account-side port forwarding and DDNS are out of scope. Advanced routing, forwarding, DNS and transport tuning are deliberately kept out of the UI; see [scope](PARITY.md).
