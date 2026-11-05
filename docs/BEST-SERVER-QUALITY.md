# Independent Best server quality assessment

2026-11-05. Linux baseline `416c1ad` (production `c52434f`), Android
`b1a612a`. No agents, PR, push, Android checkout edits or emulator interaction.

Practical objective: choose a responsive, reachable server with reasonable load
and nearby routing, honoring explicit country/server choices. This is a useful
heuristic, not proof of universal optimum. RTT regret below means selected RTT
minus the fastest known healthy candidate's RTT, not a throughput or reliability
score. Unknown RTT cannot have an honest measured regret.

## Policy and sample coverage

Linux GUI uses `desktop_score`, not CLI `Manifest::best`: base/speed +
load*load_factor + occupancy*users_factor/1000 + RTT + max(load-60,0)^2,
minus the optional capacity preference. Android uses base/speed + load/load_factor
+ occupancy/users_factor + RTT. Thus even at default manifest factors the
occupancy weight differs by 1000, and only Linux has the congestion knee.
Capacity is advertised capacity; load is aggregate bidirectional bandwidth as a
percentage. Neither establishes the client's available bandwidth. Occupancy can
exceed 100% (it did in the live manifest); user count is a weak proxy for load.
A nondefault manifest load factor also moves the two scores in opposite directions.
Linux truncates/clamps scores; Android uses doubles. Large capacity preferences
can flatten Linux scores to zero and leave alphabetical ties.

Linux samples at most 32 locations, four concurrent TCP443 connects to first IPv4
entry, max five seconds each, 30-minute refresh / five-minute failure cooldown,
24-hour retention. Representative choice rotates by seed; unsampled/old locations
come first. Android samples at most 24 locations, four workers, countries spread
before second cities; priority and low score pick a location representative.
Default core cadence is three minutes and TTL ten minutes. The app additionally
binds probes to an underlying network and shares its admission budget. Both stop
probing during tunnel activity. These are entry TCP estimates, not WireGuard UDP
entry-index measurements, exit routing, sustained jitter or application latency.
Linux's cache survives restarts; Android's samples are memory-only.

Linux inherits the minimum successful result from the same country/city; Android
publishes a successful location estimate for each member. A city may contain
multiple providers/routes, so the estimate does not establish peer health. Both
use the fleet median for unknowns (zero latency contribution if none measured).
Neither sampler knows where the user is. With more locations than the budget,
the nearby location can be omitted; a median-valued remote low-load server can
win. `bounded_coverage_cannot_guarantee_the_nearby_location` reproduces that
coverage limit and verifies explicit priority admission. Balanced full samples
select SG for a Singapore network: there is no unconditional Netherlands bias.

Linux gives failed TCP probes a finite 10,000-ms penalty, drops all-failed sweeps
as offline, and applies recent connection-failure penalties before score.
Very large base/capacity preferences can defeat that finite penalty. Android
excludes failed probes for Best, allows an explicit-target fallback and tries
up to three candidates for non-server choices after handshake failures. Neither
proves that TCP refusal means UDP failure. Linux warnings have no score but can
still be selected if all candidates have warnings; Android Best filters them.
Both filter unavailable servers. No tuning of these tradeoffs was made here.

Linux actual `Wave::selected` tests verify Best, explicit country/server, recent
failure demotion and `locklast` reconnect retention. Allow/deny countries constrain
the GUI list. Android's actual `Scoring.rank` tests verify explicit NL constraints;
connection wiring honors the target and retains the actual handshaking candidate.
Android reconnect can re-rank a changed snapshot; Linux `locklast` deliberately
retains its previous choice. A pinned target is not an automatic global Best.
Prior emulator marker/connect evidence is accepted, not repeated as quality proof.

## Reproducible representative scenarios

`tests/fixtures/best-networks.json` contains **invented representative user-network
RTTs**, with explicit load/occupancy and expected selections, based on the real
manifest fixture schema. They are not field latency datasets. Rust loads the
real XML fixture and invokes production Linux scoring/cache. The standalone
Kotlin tests parse Android's XML fixture and invoke its production rank function.
The JSON expectations describe the current policy, including a deliberate
policy divergence; they do not declare every current choice optimal.

| User network/scenario | Linux selection | Android selection | RTT regret, ms |
|---|---|---|---|
| Chicago / Amsterdam / Singapore / Tokyo, equal load | US / NL / SG / JP | same | 0 each |
| Singapore, SG 80%, other regions healthy | JP | JP | 63 |
| US unreachable | CA | CA | 0 among answering peers |
| US partial results | US | US | 0 among known peers |
| SG missing, high SG load | JP | JP | 0 known; true SG regret unknown |
| No measurements | CA alphabetical tie | CA alphabetical tie | unknown |
| SG 12 ms/80%, NL 165 ms/10%, other regions 90% | NL | SG | Linux 153; Android 0 |

The last case reproduces a Singapore-to-Netherlands outcome even with full
coverage. Linux chooses lower reported load at a **153-ms** latency cost; Android
accepts higher load. Without throughput/queueing evidence, neither decision proves
better end-user performance. Keep this counterexample as a policy limit; do not
claim the Best label promises lowest RTT. The 63-ms Japan choice is a practical
regional healthy alternative, likewise not measured superior throughput.

Commands (offline except the explicitly ignored live test):

```sh
cargo test --locked --test best_quality -- --nocapture
cargo test --locked --bin wave-vpn-gui best_gui_cases
python3 scripts/test-best-android.py /path/to/android-checkout
```

The Android runner creates an isolated temporary copy of core/build configuration,
uses cached Gradle dependencies and deletes it. It never changes the supplied
checkout. `tests/BestQualityTest.kt` can also be copied into Android core tests by
the parent. Both suites assert the recorded live five-server selection too.

## Concrete defect and minimal correction

Linux `LatencyCache::get` and the unknown median used expired in-memory samples
until a successful sweep pruned them. Offline/failed sweeps and long sessions
could keep yesterday's fastest server (or refusal) indefinitely. The new
`expired_in_memory_samples_do_not_survive_failed_refresh` failed on `416c1ad`
(`Measured(1)` rather than `Unknown`) and passes after read-time age checks in
both own/location estimates and median ranking. Sample budgets/cadence and score
weights are unchanged. Representative and live selections are unchanged; expired
data becomes unknown rather than a false 1-ms recommendation.

Precise Android proposal: **no corresponding expiry patch**. `LatencyCache.cached`
already filters samples by TTL and current entry address on each connection read.
Parent should apply/run the supplied Kotlin tests, without changing its weights
on the strength of this single access-network sample. A future shared policy
should explicitly choose a latency-versus-congestion objective, separate measured
from estimated confidence, and validate user-network outcomes before harmonizing
occupancy and congestion terms. Linux still lacks Android's per-entry cache
invalidation: a reused server name can retain a previous route's RTT until expiry.

## Bounded authorized field sample

The existing private fixture authenticated on the Mac once; five generated
profiles crossed SSH stdin only to dedicated Sigma113. Sampled one low-scoring
healthy candidate in each of US, CA, NL, SG, JP, **not a fleet-wide ranking**.
No account mutation, raw account data, keys or profiles were logged/committed.
`tests/best_live.rs` generates at most five regional private requests into a
new mode-0600 file, controlled by `WAVE_BEST_OUTPUT`; credential JSON is stdin.
`tests/best_live_linux.py` consumes it under `WAVE_LIVE_TEST_VM=1` as root in a
disposable Linux VM, reusing existing `live_linux.py` namespace/NAT cleanup.
It configures kernel WireGuard directly to isolate path quality, not GUI behavior.
This does not requalify the GUI or Android backend.

Per server: three TCP443 handshakes, one initiating ICMP packet, at most four
seconds of handshake observation, then five ICMP packets to the profile's internal
DNS address. No check-service HTTP calls, DNS challenges, downloads or throughput
transfers. Handshake observation includes initial ping completion, not a precise
WireGuard handshake timer. Public aggregate snapshot only is persisted in
`tests/fixtures/best-live-public.json`; server metadata can change.

| Public server / region | Load | TCP median ms | Tunnel ICMP median ms | ICMP spread ms | Handshake / ICMP replies |
|---|---:|---:|---:|---:|---|
| Guniibuu / US | 18% | 26.7 | 26.0 | 0.7 | yes / 5 of 5 |
| Kornephoros / CA | 29% | 35.3 | 34.6 | 0.5 | yes / 5 of 5 |
| Piautos / NL | 35% | 113.1 | 113.0 | 1.0 | yes / 5 of 5 |
| Luyten / SG | 38% | 194.0 | 194.0 | 1.0 | yes / 5 of 5 |
| Taphao / JP | 38% | 129.1 | 128.0 | 2.0 | yes / 5 of 5 |

Both actual scoring functions select **Guniibuu** within this sample, before and
after the expiry correction: **0-ms observed RTT regret**. All five handshook;
there is no observed connection-success superiority to claim. Five ICMP packets
are only a short stability observation, not long-run loss evidence. Throughput
is **unmeasured**; advertised 20-Gbit versus 2-Gbit capacity proves no client speed
advantage. No inference about other users/ISPs/regions or hardware performance.

The guest started from stopped, initially waited on network-online, and the
existing guest-agent recovery stopped that waiting unit for this boot. Cleanup
verified empty nftables rules/netns/WireGuard interfaces, restored forwarding to
zero and the resolver's original stub symlink. Guest staging source was removed (a root-owned Python bytecode cache remained
in guest `/tmp`, which clears at boot) and the guest returned to stopped. No other guest was touched.

## Recommendation and checks

Ship the small expiry correction; retain Best as a bounded heuristic. Do not
advertise universal optimality, measured throughput superiority, or a fresh
per-server health guarantee. Treat the Linux SG→NL example and unmeasured-location
coverage as known limits requiring broader controlled evidence before retuning.

Validation: baseline expiry reproduction failed as expected; corrected Rust
scenario/GUI and complete all-target suites, fmt, clippy and release binaries
were checked on macOS. Android standalone JVM suite passed against `b1a612a`.
Native Linux evidence in this review is the five-path WireGuard/ICMP harness;
prior full VM/emulator gates are documented separately. No native all-target
rerun of this expiry-only correction and no emulator quality retest claimed.
