# Wave VPN for Linux 0.1.0

**Early release** · Ubuntu 24.04 x86_64 (GNOME, systemd) · GPL-3.0-or-later

The first public build of an unofficial WireGuard client for AirVPN, written in Rust.

**v0.1.0 is an early release.** Anyone is welcome to use it, but it hasn't been fully tested on every device and system yet. Everything tested so far is in [verification](VERIFICATION.md).

## What's in it

- Sign in with your AirVPN account; credentials in the desktop keyring (Secret Service)
- Best-server choice, server search and favorites, connect and reconnect
- **Network Lock** firewall, a small privileged helper, and **Recover**
- IPv4, IPv6 and DNS checks against AirVPN on every connection
- Tray indicator, connection log, and a support report with nothing private in it
- Read-only import of your Eddie account

## Good to know

If you turn on Network Lock, it **stays on after Disconnect and Quit**. That's deliberate. To get ordinary internet back, turn it off in Settings or run `sudo wave-vpn lock-disable`. A lock that survives reboot is a separate opt-in.

## Downloads

| File | What it is |
| --- | --- |
| `wave-vpn-0.1.0-x86_64-unknown-linux-gnu.tar.gz` | Binaries, dependency notices, build provenance |
| `wave-vpn-0.1.0-source.tar.gz` | Exact tagged source, including the installer and uninstaller |
| `…-SHA256SUMS` and `…-SHA256SUMS.asc` | Checksums and their OpenPGP signature |

Verify the signature with the fingerprint in [SECURITY.md](../SECURITY.md), install the runtime dependencies, then run the installer with `sudo` ([README](../README.md#install)).

No account data, private profiles or signing secrets are published. There's no production support or security response-time commitment.
