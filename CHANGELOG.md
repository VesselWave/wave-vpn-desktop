# Changelog

## 0.1.0 · 2026-11-02 · early release

First public build. [Release notes](docs/RELEASE-v0.1.0.md).

**Added**

- WireGuard client for AirVPN, written in Rust with Iced: sign-in, servers, Network Lock, status, logs and tray
- Secret Service account storage and read-only Eddie profile import
- Root-owned socket helper, firewall-state reconciliation, crash recovery, cancellable setup and retry backoff
- AirVPN IPv4, IPv6 and DNS checks; reviewed support report you can preview and export
- Source and binary release packaging with checksum and signature verification

**Status**

Authenticated and isolated network tests pass in the environments in [verification](docs/VERIFICATION.md).

**Not planned for now:** boot autoconnect, an advanced networking UI, other protocols and other desktop VPN backends.
