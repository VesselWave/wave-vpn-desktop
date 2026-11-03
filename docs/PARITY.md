# Scope

Wave VPN for Linux deliberately does a few things and covers them properly. On 2026-10-21 the maintainer replaced the goal of full Eddie parity with this focused scope. The old full-parity inventory is superseded, not a backlog.

## In scope

- **Account:** sign in and sign out, saved automatically in the keyring. Without a keyring, a clear warning before plain-text storage in a private file. Sign-out removes saved credentials.
- **Eddie import:** read-only, into the sign-in form for review, including keyring-protected profiles. Never changes Eddie's files or keyring items; advanced Eddie settings are left out.
- **Servers:** best, country or specific server; search and favorites; connect, disconnect, switch and automatic retry, with accurate connection and protection status.
- **Network Lock:** stays on after Disconnect, cancel, Quit, failures and crashes. Recover removes tunnel resources and keeps an enabled lock; a lock you turned off stays off. The toggle works during a live tunnel and your choice carries over to future connections.
- **Status:** traffic stats, logs, tray, appearance and transfer units.
- **One-time setup:** a single administrator authorization installs the networking helper, after which the GUI never asks for a password. This helper is separate from the optional boot lock.

## Settings

Two tabs, **General** and **Network lock**: the Network Lock toggle plus five preferences (start with last server, appearance, transfer units, auto-reconnect, LAN access). LAN and ping exceptions default off. Older hidden routing, allowlist, forwarding and tuning preferences reset to safe defaults when the GUI loads them; the worker still validates them for CLI use.

Under Network Lock, only the tunnel, AirVPN's pinned sign-in endpoints, loopback, DHCP and IPv6 neighbour discovery get through by default. LAN access is an explicit opt-in; ordinary internet and external DNS stay blocked.

## Not in scope (for now)

- Boot autoconnect, and a UI for the boot lock (the CLI stays for existing installs; an ordinary lock doesn't survive reboot)
- Custom routes, DNS, backends and forwarding; MTU, timeout, port and scoring tuning
- AirVPN account-side port forwarding, DDNS and blocklists
- OpenVPN and obfuscation; other desktop VPN backends

## How the scope is verified

- Rust tests on all targets, strict Clippy, and a standalone macOS UI build
- Linux packet tests: traffic through the tunnel; IPv4, IPv6 and DNS blocked after Disconnect and Recover; only an explicit lock-disable restores internet; failures, crashes, switching, repeated cycles, and unrelated firewall rules left intact
- Ubuntu GNOME with a working Secret Service: saving, loading and deleting a test account, and importing a real installed Eddie profile
- GNOME UI: both settings tabs, sign-in and import, connection controls, protection status and layouts
- Real AirVPN backend on two servers: dual-stack internet, AirVPN DNS, switching and reconnect, protected disconnect, crash and Recover, and explicit unlock, with no DNS, DoT or direct HTTPS leaks in physical capture

Still open: full GUI retry and cancel coverage, Wayland, reboot and suspend. See [release gates](RELEASE-GATES.md) and [verification](VERIFICATION.md).

Provider checks, firewall-state reconciliation, recovery and backoff, the support report and signed-candidate verification were added after the [feature review](qa/DESKTOP-FEATURE-REVIEW.md), without widening this scope.
