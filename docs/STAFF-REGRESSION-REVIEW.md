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

## Resolution — fix pass

Fix pass on the same branch (2026-11-05). The four known-bug cases no longer carry `#[ignore]` and pass with their original assertions. Two call-site adjustments, neither a weaker expectation: the R3 case passes the stored exemptions to `firewall::policy`, whose signature now requires them (an input that did not exist was the finding); the two schedule tests that pinned the exact 300 s/1802 s boundary with jitter unit 0.5 now use unit 0, the earliest possible deadline under the new positive-only jitter, and `refresh_failures_back_off_and_recover` now requires gaps of at least 5 min (was 4) and a success deadline of exactly `next_update` (was 0.8×).

* **R1.** `verify_at_connect` retries only `Unavailable`; `Failed` returns at once. `Outcome::latch` and `Results::merge` never downgrade a failure, `verify_once` fails without further requests when its shared results already hold one, and the Linux monitor keeps one `Results` per connection. The worker reports `VerificationFailed` through `transport::failure_event` as `retryable:false, category:"verification"`; cleanup still runs, and the GUI merges `checks` events so the failure stays visible and does not auto-reconnect.
* **R2.** `schedule::jitter_above` spreads refreshes over [d, 1.2 d]. Success schedules `max(option, next_update, 5 min)` plus jitter; failures wait at least 5 min plus jitter, and a failed manual refresh never moves the automatic deadline earlier. Contract: the floor and `next_update` are strict minimums; retries back off from 5 min after an attempt that already honoured `next_update`. Reconnect jitter stays ±20 %.
* **R3.** `bootstrap::Exemption {ip, port}` with `validate`/`validate_exemptions` (at most 16; public unicast only; no unspecified, loopback, private, link-local, CGNAT, multicast, broadcast, reserved, ULA or IPv4-mapped; no port 0/53/853). `firewall::policy` requires the list and refuses unsafe input; embedded endpoints remain the only implicit exemptions (the client's fallback). Hostname endpoints are resolved by the unprivileged client (bounded 3 s, at most four public addresses per host, previous addresses kept on failure), stored with the bootstrap and pinned through `Transport::post_pinned`, so no DNS is needed under lock; the helper never sees names. The desktop sends exemptions in the worker `Request`, in `LockInput` for lock actions, and with the new `lock-update` service action when a refresh changes them while a lock is active (deferred until no connection is running, never prompting for helper installation; also once after a boot-restored lock is confirmed). The helper stores them in `/etc/wave-vpn/lock.json` (boot restore) and `/run/wave-vpn/bootstrap.json` (re-application by cleanup/Recover), and replaces the nft table in one transaction. Legacy lock files and bare-options input still parse.
* **R4.** The client rechecks cancellation after every transport result and before persisting. `BootstrapStore::save_fenced` holds a process-wide lock and an exclusive `flock`, checks that the session is current immediately before writing, and refuses a bootstrap from a manifest older (`time`) than the stored one. `wave_protocol::cancel_requests` takes the same process lock, so after sign-out returns no write from that session can land. Store format v2 adds `issued` and `resolved`; v1 files still load.
* **Probe load.** Each run probes at most `SAMPLE_BUDGET` = 32 servers: an explicitly selected server, then one seeded-random representative per location, unmeasured or longest-unmeasured locations first, ties ordered by seed rather than name. Results merge per server and expire after 24 h; unprobed servers show their location's best result. Results, measurement time and cooldown persist in `latency.json` (server names, milliseconds and Unix times only; future, expired or tampered entries ignored; cooldown capped at 30 min). The cooldown is saved when a run starts, so restarts during or after a run do not probe again; sign-out drops results but keeps the cooldown. Probes are non-blocking connects abandoned within 50 ms of cancellation, and none start while connecting, connected, reconnecting, changing the lock or under lock. Explicit selection does not depend on latency data.
* **Account refresh assessment.** Before: automatic refreshes never fetched the account, so a device removed or a subscription changed on the website stayed stale until a manual refresh. Even then, a removed selected device left Connect failing with "Select an available device". Now a manual refresh (60 s limit) recovers immediately and re-selects an existing device without sign-out. Automatic refreshes add the account document at most once per 24 h (`schedule::ACCOUNT_INTERVAL`); ordinary automatic refreshes stay manifest-only. An account refusal during an automatic refresh is shown to the user. Not done: a handshake timeout caused by a revoked key is not diagnosed as such; reconnect backoff (at most 2 min) continues until a refresh.

New tests: `tests/fix_regressions.rs` (16), `fix_*` cases in `tests/bootstrap.rs` (9) and `tests/support/fix_gui_cases.rs` (11, real `Wave::update`/`transport_line`/`privileged` wiring with a test hook in place of the helper process). Mutations (removing the stale-manifest fence; replacing the GUI status merge with overwrite) were each caught, then reverted.

Validation on macOS ARM64: `cargo test --locked --all-targets` (all pass, 5 pre-existing live/authenticated tests ignored), `cargo clippy --locked --all-targets -- -D warnings`, `cargo build --release --locked --bins`. `cargo clippy --locked --target aarch64-unknown-linux-gnu --all-targets -- -D warnings` passes as a type check only (stub libc headers for `ring`'s build script; nothing linked or executed).

Remaining limitations: Linux helper behaviour (`lock-update`, session/boot exemption files, worker failure event, nft transaction, boot restore) was compiled but not executed. The isolated network suite, reboot, authenticated live suite and GUI timing still need the separate VM gate. Hostname resolution uses the system resolver, so a lock enabled before the first manifest with a new hostname is resolved allows only previously resolved addresses. Location estimates assume co-located servers have similar latency; best-server quality is not assessed here. The 30-minute cooldown uses wall-clock time across restarts and is ignored when the clock moves backwards.
