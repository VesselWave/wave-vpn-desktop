# Changelog

## Unreleased

Lighter on AirVPN's infrastructure, following AirVPN staff feedback.

**Changed**

- Tunnel and DNS checks run once per connection instead of every 60 seconds; an unavailable check service leaves the connection up and unverified instead of disconnecting
- Latency probes: at most four at a time, cached for 30 minutes, never while connecting, connected or under Network Lock; unreachable servers rank last instead of counting as 0 ms
- Sign-in uses the bootstrap addresses and RSA key from the latest server list, saved locally, with the built-in set as a fallback; failover pauses grow between addresses
- Server list refresh: manifest only, every 30 minutes or AirVPN's advertised interval and never more often than every 5; manual refresh at most once a minute; failures back off up to an hour; reconnects are spread with random jitter
- Latency probes cover at most 32 servers per run, one per location in a random order, instead of the whole fleet; results and the 30-minute cooldown are kept across restarts (`~/.config/wave-vpn/latency.json`, server names and milliseconds only); unprobed servers show their location's result; a probe is abandoned within 50 ms when a connection starts
- Automatic refreshes add the account document about once a day; a manual refresh still always includes it. A device removed from the account is replaced by an existing one instead of requiring sign-out

**Fixed** (independent staff-regression review, [details](docs/STAFF-REGRESSION-REVIEW.md#resolution--fix-pass))

- An observed DNS or identity mismatch is final for that connection: a later unavailable or successful check can no longer turn it into "unverified" or "verified", the status keeps showing it, and the connection is stopped without automatic reconnect
- Jitter can no longer schedule an automatic refresh before the 5-minute floor or AirVPN's `next_update`; it only adds up to 20 %. A failed manual refresh no longer moves the automatic schedule earlier
- Network Lock follows bootstrap rotation: the latest manifest's validated sign-in IP/port pairs (and resolved addresses of sign-in hostnames) are passed to the helper and applied to the session and boot locks; unsafe addresses and DNS ports are refused
- Cancelling sign-in or refresh (sign-out) now also stops a late response from being saved, and a response from an older manifest can no longer overwrite a newer saved bootstrap

**Fixed** (independent recheck, [details](docs/STAFF-REGRESSION-RECHECK.md#resolution--repair-round-2))

- Network Lock bootstrap exceptions apply only to the authorized desktop user's apps, are shown in the Network Lock settings and diagnostics, and are documented as bounded but not provider-verified
- Client pinning and the helper's exceptions use one shared 16-address selection (every sign-in hostname keeps addresses); under Network Lock outside the tunnel an unpinned hostname is skipped instead of looked up through ordinary DNS
- A failed Network Lock update is no longer treated as applied: the previous set is kept and the update is retried while idle with backoff (30 s to 15 min), with reinstall advice for an older helper
- A late response with the same or an unknown manifest time can no longer replace a configuration saved after its request started, in any process
- Exceptions refuse every non-public address, including IPv6 site-local, discard-only, documentation, 6to4, Teredo and NAT64 ranges
- Simultaneous app instances share one latency probe cooldown; adopting a manifest spends at most about 9 s resolving sign-in hostnames

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
