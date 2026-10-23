# Changelog

## Unreleased — 0.1.0

- Independent GPL-3.0-or-later Rust/Iced WireGuard client for AirVPN with compact
  account, server selection, Network Lock, status/logs and tray workflows.
- Secret Service account storage and read-only Eddie profile import.
- Root-owned socket helper, firewall-state reconciliation, crash recovery,
  cancellable setup and retry backoff.
- Provider IPv4/IPv6 identity and DNS checks, redacted diagnostic preview/export,
  and source/binary release packaging with signature/checksum verification.

Authenticated backend and local-peer tests pass in the environments documented
in [VERIFICATION.md](docs/VERIFICATION.md). Remaining GUI/platform/lifecycle gates
are tracked in [RELEASE-GATES.md](docs/RELEASE-GATES.md).

No public production release has been published. Boot autoconnect, advanced
networking UI, additional protocols and other desktop VPN backends are deferred.
