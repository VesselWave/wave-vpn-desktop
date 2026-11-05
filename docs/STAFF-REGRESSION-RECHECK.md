# Independent recheck of the R1–R4 fix pass

Reviewed `1a53b9e` on `t3/linux-regression-review`, with `dbdcbaa` and `e079170` confirmed ancestors. Date: 2026-11-05. Read AGENTS.md, CONTRIBUTING.md and the complete STAFF-REGRESSION-REVIEW.md including its resolution. Only tests and documentation changed. No agents, devices, real AirVPN requests, privileged operations, VM starts, push or PR.

**Assessment: the four original reproductions pass without weakened assertions, but the fix pass is not clean.** The following five issues remain. Tests named `observed_*` deliberately pass by demonstrating the unsafe/limited behavior; they are findings, not acceptance requirements. After a production fix, replace their characterization assertions with the intended invariant. All original R1–R4 tests remain always enabled.

## Reproducible findings

### F1 — Medium, Network Lock policy integrity: arbitrary public TCP exemptions

Paths: `src/bootstrap.rs:43`, `src/transport.rs:47`, `src/firewall.rs:51`, `src/platform/linux.rs:1016`, `src/platform/linux.rs:1167`.

Reproduce: `cargo test --locked --test independent_recheck observed_helper_accepts`.

A structured lock input containing `1.1.1.1:443` is accepted without any provider manifest, key or endpoint evidence, and produces `ip daddr 1.1.1.1 tcp dport 443 accept` ahead of the output drop rules. Connect, lock-enable and lock-update all accept the same validated list. The rule permits arbitrary TCP payloads from every local process to that pair outside the tunnel, including HTTPS/DoH; excluding ports 53/853 does not prevent DNS over HTTPS. Up to 16 caller-chosen public pairs can be installed and retained at boot. This is a concrete policy escape, not a claim that all internet traffic or UDP becomes allowed.

Threat boundary: the service authenticates the installing UID with SO_PEERCRED. That UID already has lock-disable and user-configured allowlists, so this is **not** an unauthorized-UID root escalation. Root-only persistence prevents other UIDs editing the helper's state. However, calling these entries “only encrypted-bootstrap” or “validated latest-manifest” exemptions overstates what the privileged boundary establishes. A compromised desktop process or tampered user bootstrap store can supply arbitrary pairs. Synthetic policy generation proves the rule, not packet delivery.

Proposed fix: explicitly decide whether the desktop UID is trusted to relax Network Lock. For a stronger provider-only boundary, carry verifiable endpoint provenance to a trusted validator and constrain exemptions there; do not infer provenance from public-address syntax. Scope permitted traffic to the required client identity where feasible. If trusting the UID is intentional, document the actual guarantee and make these exceptions visible to the user.

### F2 — Medium, rotation availability: helper and client disagree on pinned addresses

Paths: `src/bootstrap.rs:248–283`, `src/client.rs:219`.

Reproduce: `cargo test --locked --test independent_recheck observed_hostname_pins`.

Five valid bootstrap hostnames with four public addresses each yield 20 pinned client sockets, but `exemptions()` silently truncates to 16. All four addresses of the fifth hostname are used by `pinned()` yet absent from the helper's policy. If only that hostname remains reachable, rotation fails under lock even though its validated stored configuration is usable without the lock. Existing one-host pinning tests miss this cross-endpoint bound.

Proposed fix: derive one globally bounded endpoint/address selection used by both HTTP pinning and helper policy. Reject or deliberately select an over-budget manifest rather than keeping client-only addresses. Also fail closed instead of falling back to ordinary DNS for a hostname with no stored pin if the contract requires identical pinned addresses under lock (`Client::request` currently uses ordinary `post` when `pinned` returns None).

### F3 — Medium, rotation recovery: failed helper update is permanently acknowledged

Paths: `src/gui.rs:1276`, `src/gui.rs:1490`, `src/gui.rs:804` (Notice handler).

Reproduce: `cargo test --locked --bin wave-vpn-gui observed_failed_lock_update`.

The real controller sets `applied_exemptions` before spawning/waiting for the helper. After an injected helper failure the error is shown and busy clears, but a later refresh with the same bootstrap makes no second lock-update call: the equality guard treats the failed attempt as applied. The pending flag is false. An nft failure, transient operation-lock conflict or incompatible installed helper can thus leave the active lock blocking the rotated endpoints until another rotation/relaunch/action. The test uses the existing helper-process hook; static inspection confirms the production branch sets the same state before spawn.

Proposed fix: distinguish pending/sent/confirmed endpoint sets; acknowledge only a successful operation associated with its generation. On error preserve the last confirmed set and arrange a bounded retry while idle. Avoid an unbounded retry on every UI tick.

### F4 — Medium, persistence ordering: equal/unknown times do not fence old sessions

Path: `src/bootstrap.rs:498–518`.

Reproduce: `cargo test --locked --test independent_recheck observed_equal_or_unknown`.

Different bootstrap data with the same `issued` value replaces the stored configuration; this includes absent times (0). File locking serializes writes but provides no session ordering for ties. An older uncancelled process completing after a newer one can replace its configuration/resolved pins if their times tie; the same manifest's hostname resolution can differ even with identical URLs/keys/time. The strictly-newer timestamp test passes across eight actual child processes, but that is a narrower guarantee than all stale generations being fenced.

Cancellation through `cancel_requests` is correctly serialized against writes sharing its atomic flag within one process. It does not cancel another application's process or provide a shared session generation. No contradiction is claimed for a cancelled flag that actually belongs to the writer. Independent timestamps and absent timestamps are tested directly rather than claiming the provider necessarily rotates different keys at an equal time.

Proposed fix: add a durable request/session generation or compare-and-swap store revision captured before the request, coordinated under the file lock; define tie and missing-time handling explicitly. Retain monotonic manifest-time protection as an additional check. Preserve legitimate hostname re-resolution while rejecting an older request's late write.

### F5 — Medium, validation contract: non-public IPv6 accepted as public

Path: `src/bootstrap.rs:70–78`.

Reproduce: `cargo test --locked --test independent_recheck observed_non_public_ipv6`.

`fec0::1` (deprecated site-local) and `100::1` (discard-only prefix) both validate and generate TCP exemption rules. The predicate rejects selected special ranges rather than establishing globally routable public unicast. Site-local traffic can escape the intended public-only boundary on networks that route it; discard-only acceptance demonstrates the validation gap, not useful public reachability. Existing rejection tests do not include either category.

Proposed fix: define and test the complete special-address policy for both families, including transition/translation ranges and their effective destinations. Keep offline fixtures for exceptional prefixes; do not replace this with DNS-name validation.

## Verified scope and remaining boundaries

* **Assertion audit:** diffed `dbdcbaa..1a53b9e` for the original cases. Four finding ignores were removed. R3 supplies the newly required exemptions argument. Earliest-boundary schedule tests use unit 0 instead of 0.5 under positive jitter. The retry floor was strengthened from four to five minutes. No original mismatch, refresh-floor, rotation-rule or cancelled-response assertion was relaxed. Exactly five existing keyring/Eddie/live tests remain ignored; none is R1–R4.
* **R1:** Linux monitor uses the shared result object across its bounded connect-time passes, aborts on Failed before connected, cleans touched resources, emits a nonretryable verification failure, then GUI End avoids reconnect and keeps failure text. The new permutation test checks failure in each of three retry positions, later success/unavailable/stopped, and cancellation racing the failure. Status merges latch failures. A GUI `state:stalled` resets checks, but production cannot emit it after a terminal mismatch because monitor has returned. No synthetic event permutation is represented as actual Linux execution.
* **R2:** strict five-minute floor and next_update on successful refresh scheduling; positive-only jitter, running/manual admission, failure backoff and manual-failure preservation are exercised. Reconnect jitter remains independent. Manual refresh intentionally has a separate 60-second cooldown. Retry after a failed already-due automatic attempt need not wait the last manifest's full next_update again; this matches the documented resolution contract.
* **R3 production trace:** GUI Connect serializes `Request.bootstrap`; privileged actions serialize LockInput; worker validates it before configure; configure/cleanup/recover and boot restore call the same policy generator. `set_firewall` replaces delete+create through one `nft -f -` batch. Session state and boot configuration are separately atomic files, not one atomic filesystem+nft commit. Cleanup retains journal/protection on recovery errors. Empty explicit lists in several paths mean “reuse session/persistent list”, not “clear”; only lock-update has unambiguous empty-list replacement. Actual rollback, power loss, rule enforcement and reboot remain VM gates.
* **Compatibility:** legacy persisted locks and bare-options CLI input still parse. The installer refreshes both the persistent-lock executable and the service helper, restarting the service. Merely launching a new GUI alongside an old installed helper does not negotiate or reinstall: `service::command_cancellable` only installs when the socket is absent. Old Request/Options deny-unknown-field parsing rejects the new structured input, and the old action allowlist lacks lock-update. Use the documented installer for upgrades; test old-helper/new-GUI rejection, reinstall, persistent lock preservation and reconnect in the VM. No claim of automatic compatibility recovery.
* **R4 filesystem:** fresh files are 0600, directory 0700; symlink store loads are ignored and atomic save replaces the link without modifying its target; lockfile O_NOFOLLOW rejects a symlink; a final-directory symlink is rejected. Tests do not establish hostile same-UID concurrent ancestor replacement, hardlink ownership guarantees or power-loss durability (directory fsync is absent). Helper private files use root ownership/mode checks and O_NOFOLLOW; reviewed statically only.
* **Probes:** 32-server representative budget, four concurrent probes, bounded timeout/cancellation, seed distribution, priority server, persisted restart state and sign-out cooldown have tests. A new real-controller test confirms an explicitly selected Unreachable server still produces a profile; the existing Unknown-selection test is retained. Best-server quality is explicitly deferred. Thirty minutes applies after complete measurements; an interrupted/crashed/failed run persists the five-minute retry interval at start, not a universal thirty-minute interval. Disk cache is shared without cross-process admission locking, so simultaneous app instances are not proven to share a global traffic budget.
* **Account/UX:** existing real-controller tests exercise removed-device reselection, manual recovery without sign-out, automatic account refusal display, stale event rejection, cached profiles after refresh failure and busy-state probe suppression. New F3 test checks error display and busy reset after helper failure. Account refresh is at most daily on successful automatic recovery, with manual recovery available after 60 seconds. Tests establish state transitions, not measured interactive responsiveness, unlocked keyring, dialogs or authenticated success. HTTP cancellation still waits for blocking transport timeout; up to 16 hostname lookups can each wait three seconds when adopting a manifest.

## Added tests and mutation evidence

Ten new enabled test functions: seven in `tests/independent_recheck.rs` (including the child-process entry), two in `tests/support/fix_gui_cases.rs`, one in `tests/fix_regressions.rs` (18 terminal-mismatch combinations). Reproduce independently:

```sh
cargo test --locked --test independent_recheck
cargo test --locked --test fix_regressions recheck_mismatch
cargo test --locked --bin wave-vpn-gui observed_failed_lock_update
cargo test --locked --bin wave-vpn-gui recheck_explicit_failed_probe
```

Three sequential temporary production mutations were caught with exit 101 and restored byte-for-byte:

1. Retry Failed verdicts in `verify_at_connect`: the new permutation test reports a nonterminal result.
2. Remove O_NOFOLLOW from bootstrap FileLock: the new symlink test fails its rejected-save assertion.
3. Remove the stored-issued comparison: the new separate-process test sees issued 100 instead of 107.

No production mutation is committed.

## Validation and gate

Executed on **macOS ARM64**, not Linux:

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked --bins
```

All pass after the additions and restored mutations: 123 tests passed, zero failed, five pre-existing tests ignored across 17 targets. The dependency warning for future Rust incompatibility in `block 0.1.6` remains. Raw local logs are `/tmp/wave-review-final-{tests,clippy,release}.log` and `/tmp/wave-review-mutation-{terminal,symlink,process}.log`; these are temporary evidence, not committed artifacts.

The prepared checkout is a Darwin host. Docker reports permission denied accessing its socket; the repository's Lima guests are stopped. No guest was started because the user reserved VM work for the later gate. No Linux cross-check stubs were used or counted as runtime evidence. This review does **not** establish a fresh passing Linux runtime result. Linux namespace suites (direct and broker), nft atomic failure injection, cleanup/Recover, boot restore/reboot, upgrade/reinstall, permissions under root and authenticated UX require the separate VM gate after the synthetic findings are resolved. Real AirVPN traffic and dedicated Best server assessment remain outside this review.

## Resolution — repair round 2

Task branch commit on top of `467e0f7`. Every `observed_*` characterization test now asserts the intended invariant under a `recheck_*` name. Nothing is weakened.

* **F1 (trust decision).** The authorized desktop UID is trusted to relax Network Lock: it can already disable the lock and set allowlists, so supplied pairs are not an unauthorized-UID escalation. No provider proof exists for the helper to check. Eddie's manifest isn't signed in a form the helper could verify, and only the unprivileged app holds the encrypted session. This change doesn't invent one. Instead, `firewall::policy` takes the client UID and prefixes every bootstrap accept with `meta skuid <uid>`, so other local UIDs gain no new public TCP. The UID comes from the root-only service authorization (`service::authorized_uid`), or from SUDO_UID/PKEXEC_UID for direct root CLI runs. With no known UID, supplied pairs are omitted; the compiled-in literals remain unscoped as before. SECURITY.md and the user guide state the real boundary: the helper bounds destinations but doesn't establish provenance, and a compromised same-UID process or tampered store can open up to 16 TCP destinations, DoH included. Active exceptions are shown as a count on the Network Lock page, on the overview while locked, and in diagnostics. Test: `recheck_supplied_exemptions_are_limited_to_the_client_uid`.
* **F2.** `Bootstrap::exemptions` is the single bounded selection: literals first, then hostname addresses round-robin. `resolve_hosts_bounded` keeps only addresses inside it, and `Bootstrap::pinned` filters pins against it, which also covers old stored maps. `Client::requiring_pins` skips an unpinned hostname, including the embedded `bootme.org`, instead of using ordinary DNS. The GUI sets it while Network Lock is on and no tunnel is up. Tests: `recheck_hostname_pins_are_a_subset_of_the_exemption_set`, `recheck_required_pins_skip_unpinned_hostnames`.
* **F3.** The GUI tracks the confirmed set, the in-flight set (with its generation) and the connection set separately. Only a successful `Notice` of the same generation confirms a set; a connection's set is confirmed by the worker's `protection` event. A failure keeps the confirmed set and schedules one idle retry (30 s doubling to 15 min); the tick never retries during the backoff. Outputs matching an older helper's rejections show reinstall advice. Tests: `recheck_failed_lock_update_is_retried_with_backoff_until_confirmed`, `recheck_connection_confirms_exemptions_only_after_the_worker_applies_them`.
* **F4.** The store has a durable `revision`. `Client::manifest` captures it before the request, and `save_fenced` compares it under the file lock. Rules: an older time is Stale; a strictly newer known time is written; an equal time (unknown 0 included) is written only if the revision is unchanged. Re-resolving the same manifest from the current revision stays allowed. Tests: `recheck_equal_or_unknown_manifest_time_is_fenced_by_revision`, `recheck_write_during_request_fences_an_equal_time_response`, plus the eight-process test.
* **F5.** `public_unicast` is now an allowlist: IPv4 outside every special-purpose block, IPv6 only in 2000::/3 minus 2001::/23, 2001:db8::/32, 2002::/16 and 3fff::/20. Transition and translation forms are refused rather than judged by their embedded IPv4 address. Test fixtures that used documentation ranges as "public" moved to ordinary unicast addresses (no traffic; fake transports). Test: `recheck_non_public_addresses_are_refused`.
* **Latency admission.** `LatencyStore::admit` starts a run under an exclusive lock on `latency.lock` only if no other instance's persisted cooldown is active; otherwise it adopts that cooldown. Test: `recheck_second_instance_is_not_admitted_during_another_cooldown`.
* **Adoption DNS.** No new lookup starts after `RESOLVE_BUDGET` (6 s) or cancellation, so adoption waits at most about 9 s instead of 48 s. Unresolved hosts keep their previous addresses. Lookups run on the request thread, never the UI thread. Test: `recheck_hostname_resolution_stops_at_the_budget`.

**Remaining limits.** No Linux runtime evidence: `meta skuid` matching, nft acceptance of the scoped rules, boot restore and old-helper behavior are synthetic and type-checked only, and need the VM gate. Kernel-generated packets without a full socket (e.g. TIME_WAIT ACKs) don't match `skuid` and are dropped, which only blocks more. Scoping the compiled-in literals means a root-run CLI can't reach them under Network Lock when a desktop UID is authorized. A lock installed by root CLI without the service and without SUDO_UID/PKEXEC_UID gets no supplied pairs. The old-helper hint is a string heuristic, and a new GUI with an old helper still can't connect until the installer runs. Blocking HTTP cancellation still waits for the transport timeout. The latency lock is advisory and best effort.
