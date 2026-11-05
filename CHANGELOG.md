# Changelog

## Unreleased

Lighter on AirVPN's infrastructure, following AirVPN staff feedback.

**Changed**

- Tunnel and DNS checks run once per connection instead of every 60 seconds; an unavailable check service leaves the connection up and unverified instead of disconnecting
- Latency probes: at most four at a time, cached for 30 minutes, never while connecting, connected or under Network Lock; unreachable servers rank last instead of counting as 0 ms
- Sign-in uses the bootstrap addresses and RSA key from the latest server list, saved locally, with the built-in set as a fallback; failover pauses grow between addresses
- Server list refresh: manifest only, every 30 minutes or AirVPN's advertised interval and never more often than every 5; manual refresh at most once a minute; failures back off up to an hour; reconnects are spread with random jitter

## 0.1.1 · 2026-11-03 · early release

New look; interface only. [Release notes](docs/RELEASE-v0.1.1.md).

**Changed**

- VesselWave family look shared with Wave Remote and both websites: blue-black dark theme, pill buttons, segments and filter chips, one corner scale

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
