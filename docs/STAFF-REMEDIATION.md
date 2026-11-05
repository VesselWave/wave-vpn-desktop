# Staff infrastructure remediation

[AirVPN Staff feedback](https://airvpn.org/forums/topic/82017-wave-vpn-open-source-wireguard-client-for-android-and-linux-unofficial-v011/?do=findComment&comment=268064), 2026-11-05.
Independent test authors and implementation delegates used separate worktrees
(Opus5.5 and6.1Sol); failing reproductions were promoted into ordinary offline
regressions after repair. Historical review documents retain their original
findings; the final resolution and runtime scope below supersede open-status text.

## Implemented and tested

- Provider identity/DNS verification runs at connection time with bounded retries
  for unavailable services. Observed mismatch is terminal and latched; an unavailable
  check stays visibly unverified. Connected monitoring uses local route checks.
- Bounded representative TCP latency samples, four workers, persisted admission/cache,
  no active-tunnel probes, cancellation, unreachable-last ranking and read-time expiry.
- Latest manifest bootstrap URLs/RSA persisted with fair failover and backoff.
  Manifest-only automatic refresh honors minimums, cached data survives outages,
  and manual account refresh recovers removed-device selection.
- Network Lock accepts bounded public endpoint pins through validated helper input,
  confirms updates only on success, retries failures, preserves boot/recovery state,
  scopes exceptions to the authorized UID, and atomically replaces nft rules.
  The authorized UID is trusted to relax the lock; public-address validation is not
  cryptographic provider provenance. See SECURITY.md for the exact boundary.

## Reproduce

```
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked --bins
```

Inside a disposable Linux VM:

```
sudo python3 tests/linux_network.py target/debug/wave-vpn
sudo env WAVE_TEST_SERVICE=1 python3 tests/linux_network.py target/debug/wave-vpn
```

[Native VM evidence](SIGMA-LINUX-ACCEPTANCE.md) records native tests,19 direct/21
broker checkpoints, real UID/IP/port packet tests, failed/atomic updates, Recover,
old-helper rejection, installer upgrade and actual reboot restoration. Bounded
live VPN checks and synthetic keyring checks passed. The guest was restored/stopped.
[Best quality](BEST-SERVER-QUALITY.md) records the separate five-server field sample
and the final platform-independent expiry correction, with focused macOS checks.
The full native gate precedes that small expiry-only change.

## Limits

Best is a bounded latency/load heuristic, not universal lowest RTT or throughput.
The documented Singapore congestion-policy counterexample remains; weights were
not retuned from one access network. Authenticated interactive GUI timing, desktop
keyring unlock, physical suspend and sustained reliability remain unqualified.
No release/tag or forum reply was made. Upgrade the installed helper with the GUI.
