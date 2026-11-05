# Independent Linux infrastructure regression review

Reviewed implementation: `e0791707b5d969b3b4d7b10e7f893a1399fdfe4e` (already HEAD on the prepared `t3/linux-regression-suite` branch). Review date: 2026-11-05.

Requirements were derived from [AirVPN staff comment 268064](https://airvpn.org/forums/topic/82017-wave-vpn-open-source-wireguard-client-for-android-and-linux-unofficial-v011/?do=findComment&comment=268064), retrieved with curl: avoid synchronized verification/reconnection during service outages; bound and cache latency traffic, exclude tunnels, avoid ranking failures as fastest; retain manifest bootstrap endpoints and RSA rotation with fallback. AGENTS.md and CONTRIBUTING.md were read. No production behavior was changed. `src/gui.rs` gains only a `cfg(test)` module declaration to exercise private controller wiring.

## Findings requiring parent fixes

### R1 — High: verification retries erase an observed mismatch

`src/checks.rs:276-284` retries `Verdict::Failed` and returns only the last verdict. A first DNS/identity mismatch followed by unavailable responses becomes `Unavailable`; followed by success it becomes `Verified`. Linux `src/platform/linux.rs:845-874` consumes that aggregate result, aborts only for the final `Failed`, and otherwise emits `connected`. Each pass at `src/platform/linux.rs:741` starts fresh Results, so a previous failure disappears from status too.

Reproduction: `cargo test --locked --test staff_regressions staff_observed_mismatch -- --ignored`. Actual failure: `mismatch became Unavailable`. The test also specifies the success-after-mismatch case, but stops at the first failing assertion. This is a real monitor path, not merely an unused abstraction. Proposed fix: make provider-observed failures terminal (or latch them across passes); retry only unavailable checks. Preserve failed status and stop/clean up before accepting the connection. Add a recheck for both permutations after fixing.

### R2 — Medium: negative jitter violates the automatic refresh minimum

`src/schedule.rs:85` applies ±20% jitter after selecting the five-minute floor; a random unit of zero schedules 240 seconds. GUI `src/gui.rs:807-808` uses that deadline directly. Failure retries at `src/schedule.rs:90` have the same four-minute first lower bound. A manifest interval of 30 minutes can likewise become 24 minutes.

Reproduction: `cargo test --locked --test staff_regressions staff_minimum_refresh -- --ignored`. Actual: `due(now + 299 seconds)` is true. Proposed fix: clamp the jittered deadline to the intended minimum and decide explicitly whether `next_update` is a strict minimum or an approximate hint. Keep jitter by spreading above the minimum. The existing tests deliberately permit four-minute failure delays, so agreeing the contract matters.

### R3 — High availability: Network Lock cannot follow bootstrap rotation

`src/firewall.rs:32` enumerates only `Bootstrap::eddie().urls`. Linux `src/platform/linux.rs:238-243` invokes this generator without latest-bootstrap data, including while disconnected with persistent lock. After manifest rotation retires the embedded IPs, persisted new endpoints remain blocked by the output drop policy. `Client::remembering` cannot repair this. Hostname-only new endpoints also require an explicit safe resolution policy; broadly opening DNS or all HTTP is not an acceptable substitute.

Reproduction: `cargo test --locked --test staff_regressions staff_network_lock -- --ignored`. It saves/loads a validated latest configuration with `203.0.113.77:8080`, then calls the production policy generator with LAN exemptions disabled. The required IP/port accept rule is absent. This proves the generator/input gap, not kernel nftables enforcement (deferred to VM).

Proposed fix: carry validated latest endpoint IP/port pairs through a bounded, validated helper interface; update persistent/session lock policies atomically and retain only justified embedded fallback exceptions. Test both literal families, changed ports, hostname resolution, rotation, restart and rejection of unsafe arbitrary exemptions. This needs code changes, not just documentation.

### R4 — Medium: cancellation does not fence bootstrap persistence

`src/client.rs:169-170` accepts a successful response without rechecking cancellation; `src/client.rs:177-186` then saves its manifest. GUI epoch checks at `src/gui.rs:555-558` reject stale UI results only after that write. Signing out while a request is in flight can therefore replace bootstrap state after cancellation; an old response can race a later session's newer bootstrap write.

Reproduction: `cargo test --locked --test bootstrap staff_cancelled_response -- --ignored`. The fake sets cancellation during POST and returns an authentic encrypted synthetic manifest announcing a rotated key/address. Expected previous stored bootstrap remains; actual stored URL changes to `http://203.0.113.77:8080`.

Proposed fix: check cancellation after transport completion and before accepting or persisting results; fence shared persistence against stale generations so concurrent old/new sessions cannot roll it back. In-flight blocking HTTP itself remains uncancellable until the configured timeout; test bounded user-visible cancellation separately.

## Coverage and evidence

New tests are in `tests/staff_regressions.rs`, appended cases in `tests/bootstrap.rs`, and `tests/support/staff_gui_cases.rs` (included by the GUI test target). Four known-bug cases are explicitly ignored with finding IDs so ordinary offline checks remain usable; ignored means unresolved, not passing. Run only the commands above to reproduce them; do not run all ignored tests because the existing authenticated test is live.

| Concern | Evidence and boundary |
| --- | --- |
| Probe budget, cache expiry, concurrency | Existing latency fake exercises 250 targets, at most four simultaneous probes, timeout cap and day-long rate budget. New tests cover nanosecond-before expiry, expired cache retained after cancellation, stale delivery, and 100 concurrent manual admissions under serialized schedule ownership. No sockets. The mutex model is not a test of GUI thread scheduling. |
| Probe cancellation | New fake cancels from the first probe; at most four in-flight calls drain and partial results are discarded. Existing generation tests reject stale pings. Active TCP sockets can last up to five seconds; Connect does not join their worker. |
| Bootstrap persistence and rotation | Existing synthetic encrypted transport tests actually decrypt requests with distinct RSA private keys, assert latest URL/key used after restart, fallback, refusal stops immediately versus transport failure continues. New tests cancel during failover pause, expose cancelled-response persistence, and reject missing/oversized/credential-bearing/malformed bootstrap fields without discarding the usable server list. |
| Minimum, backoff, jitter | Existing fake-clock tests cover exponential backoff/cap, reset on success, reconnect jitter and manifest hints. New burst test verifies one admission, exact manual cooldown boundaries, and first automatic retry boundary; R2 exposes floor/jitter interaction. Manual requests intentionally have their own 60-second limit rather than the full automatic outage backoff. |
| Verification outage and mismatches | Existing day-long and fleet simulations prove once-per-session scheduling and unavailable service does not cause disconnects. New cancellation-in-pause test prevents subsequent verification requests. R1 tests the retry aggregator the Linux worker actually uses. |
| Actual controller/status wiring | New tests call `Wave::update` and `transport_line`, preserving valid cached connect profiles/pings after refresh failure, refusing stale successful events, keeping pending current refresh state, distinguishing unavailable from verified, and blocking probes with lock/control/retry state. Preview mode disables real networking and persistence. |
| UX timing | Cached profile remains immediately constructible after refresh failure; no refresh thread is awaited by Connect, and probing is cancelled after profile construction. These are code/behavior checks, not measured interactive launch/connect/refresh latency. VM/UI measurement remains required. |
| Ranking | Existing tests cover unreachable cost and unknown-at-median behavior; new cache test keeps unknown display separate from ranking. Median substitution is a policy choice, not proof the best server is optimal. Dedicated assessment remains deferred. |

Two targeted mutations were applied temporarily, tested, and restored before final checks:

* Disable the production GUI Refreshed epoch guard: `cargo test --locked --bin wave-vpn-gui staff_stale_success -- --nocapture` fails with server count `0` instead of `259`. This confirms the new controller test detects removal of the real guard.
* Disable the latency worker cancellation check: `cargo test --locked --test staff_regressions staff_cancellation_drains -- --nocapture` fails its maximum-four-calls assertion (the entire queue drains). This confirms the cancellation test detects missing production cancellation, beyond checking final Outcome alone.

No mutation is committed. All four known-bug cases fail against the unchanged implementation. Their stderr is summarized above; temporary raw logs are not repository artifacts.

## Validation and limitations

Required checks run on this host (macOS ARM64):

```
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked --bins
```

All three pass. The test run reports 73 passed, zero failed and nine ignored across 15 targets; four ignored cases are the new unresolved regressions, five are pre-existing. There are 14 new cases in total: ten passing and four known failures. A pre-existing dependency warning reports future Rust incompatibility in `block 0.1.6`.

Attempted Linux compile check: `cargo check --locked --target aarch64-unknown-linux-gnu --no-default-features --all-targets`. It cannot complete because `aarch64-linux-gnu-gcc` is missing. This is not a successful Linux check. Linux worker wiring was reviewed statically; root networking namespaces, actual nftables effects, systemd, keyring, device/UI timing, VM/emulator behavior and live account operation were not executed. The parent task owns those later gates. No real AirVPN fleet traffic was generated; the only AirVPN request was the public forum-page curl.

Remaining known limitations: TCP port 443 probes still cover the whole fleet per eligible sweep (bounded concurrency/frequency, not ICMP or a reduced target budget); latency cache is process memory, so application restarts do not retain its 30-minute protection. New/unknown servers receive a measured median for ranking. Do not represent these as validated improvements in probe type, cross-launch persistence or best-server selection.

Work is local tests/documentation only, with no push, PR, credentials, private profiles, build outputs or reference code imported. Preserve GPL-3.0-or-later and existing provenance.
