# Verification

What has been tested on Wave VPN for Linux, what failed, and what's still open. The dated log below is the evidence; newer reports are in [qa/](README.md#how-it-was-tested), and what remains before a stable release is in [release gates](RELEASE-GATES.md).

## Summary

**Passes**

- Rust tests on all targets and strict Clippy, on Linux and macOS CI
- Isolated network suite (19 scenarios) with the shipped v0.1.0 binary on a physical CachyOS laptop, plus Ubuntu, Debian and Fedora lab guests ([report](qa/RELEASE-PHYSICAL-LAPTOP.md))
- Authenticated live suite from the v0.1.0 source on ARM64: two AirVPN servers, IPv4, IPv6 and DNS checks, switching, crash recovery, and no escaped packets in capture ([report](qa/RELEASE-PUBLIC-PREFLIGHT.md))
- Earlier real-backend runs on two servers: dual-stack traffic, AirVPN DNS, protected disconnect, crash and Recover
- EFI boot with the optional boot lock; keyring save and restore; Eddie 2.26.2 and 2.27.2 profile import
- Installed-release GUI: wrong-account vs unreachable sign-in, Recover before sign-in

**Seen once, not reproduced**

- A direct DNS query failed after switching to the second server in one ARM64 live run. The rerun passed in full; it's tracked in the release gates.

**Not yet covered**

- GUI connect, cancel, switch, retry and stop with the release binary; tray; sign-out; desktop login (PAM)
- Physical suspend and resume; the full install, upgrade and conflict matrix

## Log, newest first

- [Staff regression fix pass — 2026-11-05](#staff-regression-fix-pass--2026-11-05)
- [Infrastructure load fixes — 2026-11-05](#infrastructure-load-fixes--2026-11-05)
- [0.1.2 packaging and installer — 2026-11-05](#012-packaging-and-installer--2026-11-05)
- [Parent review — 2026-10-23 UTC](#parent-review--2026-10-23-utc)
- [Round 4 release verification — 2026-10-23 UTC](#round-4-release-verification--2026-10-23-utc)
- [Round 3 evidence audit — 2026-10-22](#round-3-evidence-audit--2026-10-22)
- [Desktop release requirements implementation — 2026-10-21](#desktop-release-requirements-implementation--2026-10-21)
- [Desktop presentation follow-up — 2026-10-21](#desktop-presentation-follow-up--2026-10-21)
- [Tray and one-time networking authorization — 2026-10-21](#tray-and-one-time-networking-authorization--2026-10-21)
- [Account flow and live Network Lock toggle — 2026-10-21](#account-flow-and-live-network-lock-toggle--2026-10-21)
- [Authenticated backend verification — 2026-10-21](#authenticated-backend-verification--2026-10-21)
- [Focused desktop verification — 2026-10-21](#focused-desktop-verification--2026-10-21)
- [Desktop parity update — 2026-10-21](#desktop-parity-update--2026-10-21)
- [Initial verification](#initial-verification)

## Staff regression fix pass — 2026-11-05

Fixes R1–R4 of the [independent review](STAFF-REGRESSION-REVIEW.md) and the
remaining probe-load concern. The four known-bug cases now run unconditionally
and pass with their original expectations. New deterministic guards are in
`tests/fix_regressions.rs`, `tests/bootstrap.rs` (`fix_*`) and the GUI test
target (`tests/support/fix_gui_cases.rs`, real `Wave::update` wiring); see the
review's resolution section for the list. Two mutations (removing the stale
manifest fence; replacing the GUI status merge with overwrite) were applied
temporarily and each made a new test fail, then restored.

Host (macOS ARM64): `cargo test --locked --all-targets`, `cargo clippy --locked
--all-targets -- -D warnings` and `cargo build --release --locked --bins` pass.
`cargo clippy --locked --target aarch64-unknown-linux-gnu --all-targets -- -D
warnings` passes as a type check only (stub C headers for `ring`'s build
script; nothing linked or run). The Linux helper paths (`lock-update`, session
and boot lock exemptions, worker failure event) are not executed by these tests;
the isolated network suite, kernel nftables effects, reboot restore and the
authenticated live suite still need the Linux VM gate.

## Infrastructure load fixes — 2026-11-05

Changes from AirVPN staff feedback (see [provenance](PROVENANCE.md#infrastructure-load-review--2026-11-05)),
covered by deterministic tests with fake transports, probes and clocks; nothing
contacts AirVPN:

- `tests/verification.rs`: provider checks run exactly once in a simulated
  24-hour session; a 10-minute check-service outage across 1000 simulated
  clients causes no requests from connected clients and no disconnects; an
  observed identity or DNS mismatch still stops the connection; connect-time
  retries are capped at three passes, 1 s then 2 s apart; refresh floor,
  manifest hint, manual rate limit and failure backoff over a six-hour outage;
  reconnect delay jitter.
- `tests/latency.rs`: at most four probes in flight for 250 servers, one probe
  per server, unreachable never 0 ms, all-fail treated as offline, cancellation,
  30-minute cache, no probes with a tunnel active, and at most 24 runs in a day
  of 15-second refreshes.
- `tests/bootstrap.rs`: a fake encrypted endpoint with test RSA keys covers
  first-run use of the embedded set, use and persistence of the latest manifest
  bootstrap including key rotation, invalid manifest data keeping the previous
  set and the server list, fallback to the embedded set, bounded failover
  pauses, account refusal stopping failover, manifest-only refresh,
  cancellation, and a store that ignores damaged or symlinked files.
- `tests/desktop_scoring.rs` and GUI unit tests: unreachable servers rank last;
  failed refreshes keep the cached account and servers; stale latency results
  are ignored; sign-out cancels in-flight requests; an unavailable check is not
  blamed for a later disconnect.

`tests/live_linux.py` now expects the new contract: wrong system DNS still
refuses Connected; an unreachable check endpoint (outage or one IP family)
connects unverified and stays up until Disconnect.

Host (macOS) all-target tests and strict Clippy pass, and strict Clippy passes
for `aarch64-unknown-linux-gnu`. Not yet run: the isolated Linux network suite,
the authenticated live suite, or GUI behavior in a VM.

## Initial verification

The latest focused and authenticated sections below supersede historical gaps.

2026-10-21 UTC, macOS development host.

Passed:

- Android: six offline protocol/security tests; APK debug build and lint (warnings remain).
- Android: live bootstrap decrypts AirVPN's explicit refusal of a synthetic invalid account.
- Rust: seven offline protocol/options/statistics tests, all-target build/test, strict Clippy.
- Rust: separately enabled live invalid-account bootstrap test.
- Tests cover RSA session wrapping, AES/association interoperability, official
  manifest parsing, group inheritance, entry indices, IPv6 endpoints, missing
  devices, account mismatch, XML entity rejection and profile directive injection.
- Android debug APK installed separately as `me.vwave.vpn.dev`, preserving any
  existing production installation.
- GitHub API confirms both legacy repositories remain private and archived.

Not verified:

- Android UI runtime: opening the app returned `DEVICE_IN_USE` because another
  agent-device session owns the emulator. That session was left alone.
- Authenticated account flows and actual WireGuard connections on either platform.
- Authenticated Linux AirVPN sessions and reboot/suspend/network-change behavior.
  Local-peer Linux checks are recorded below; macOS UI compilation is not VPN support.
- Leak protection, split-tunneling interactions and feature parity with old apps.

Repeat locally:

```sh
./gradlew :core:test :app:lintDebug :app:assembleDebug
WAVE_LIVE=1 ./gradlew :core:test --rerun-tasks
```

```sh
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --test protocol live_bootstrap -- --ignored
```

Live tests use only made-up credentials. No real user credentials are required.
Rust's macOS dependency `block 0.1.6` currently emits a future-compatibility warning.

After the repository split, each app was checked from its own repository root.

## Desktop parity update — 2026-10-21

Both desktop UIs build and render on Ubuntu 24.04 ARM64 in the local VM.
The new UI also builds on macOS. Rust's seven offline tests and strict Clippy
pass on both systems. The real AirVPN test remains separately opt-in.

`tests/linux_network.py` uses generated local WireGuard peers in private mount,
network and PID namespaces, with an /etc overlay. Verified: dual-stack handshake,
traffic and byte counts; blocked plaintext bypass; explicit routes/entry interface;
forwarding/NAT with blocked bypass and restored sysctls; EOF/SIGTERM cleanup;
handshake-failure/crash protection and Recover; preservation of unrelated firewall
rules; DNS file/symlink restoration, isolated systemd-resolved and resolvconf
compatibility configuration; persistent-policy restore/retention/disable.

Linux tray: registered with XFCE, menu actions present, closing hides the window
while the process runs, tray activation reopens it, and loss of the tray host
restores a hidden window. Captures compare the
archived rendering harness with the new views; all data are synthetic.

Full parity is unfinished: boot autoconnect and Eddie profile import remain.
Systemd service installation/reboot behavior and real AirVPN connections have
not been verified. See the desktop parity checklist for remaining work.

## Focused desktop verification — 2026-10-21

Supersedes the full-parity objective above. Android unchanged.

- macOS: all-target Rust tests, strict Clippy and both binaries build with
  `--locked`. Existing `block 0.1.6` future-compatibility warning remains.
- Ubuntu 24.04.4 LTS ARM64, kernel 6.8.0-134: all-target Rust tests and strict
  Clippy pass. `tests/linux_network.py` passes: actual IPv4/IPv6 tunnel packets,
  external TCP/UDP DNS blocked, Disconnect and Recover retain protection,
  explicit lock-disable restores internet, failure/EOF/signals/switch/crash,
  handshake expiry, concurrency, three repeated cycles, DNS restoration,
  unrelated firewall preservation and persistent-policy compatibility checks.
  Compatibility tests still cover routes/forwarding; these are no longer GUI features.
- Desktop upgraded from XFCE to actual Ubuntu GNOME Shell 46.0, X11 via
  TigerVNC/software rendering. Wave opens and the two settings tabs render;
  Network Lock actions and connection/LAN/reconnect status are visible. This is
  not evidence for Wayland, GDM/PAM auto-unlock, suspend or reboot behavior.
- `scripts/test-keyring.sh`: both isolated Secret Service tests pass (Wave
  account save/load/delete, Eddie v1 filename/v2 profile-ID lookup, missing item
  rejection, read-only preservation).
- Actual GNOME session: unlocked synthetic login collection confirmed with
  Secret Service `Locked=false`; store/delete succeeds. Official signed APT
  repository key fingerprint verified as
  `239FD9507903EA9FBF7DCCF1C6E338858A239AEC`; installed `eddie-ui` 2.26.2.
- `WAVE_EDDIE_DESKTOP_TEST=1 python3 scripts/test-eddie-import.py` passes:
  installed Eddie CLI loads and rewrites a synthetic v2s profile, Wave reads its
  account via the GNOME keyring, source bytes and Eddie item remain unchanged,
  test removes its own item afterward. No real account credentials used.
- Eddie GUI packaging limitation: its bundled ARM64 libgdiplus needs
  `libjpeg.so.62` then `libtiff.so.5`; libjpeg62 is available, but the latter is
  absent on Noble. Preloading current libgdiplus did not fix the bundle. The
  official package's CLI works; GUI-to-GUI import was not claimed.

Outstanding: complete interactive state coverage, real reboot/suspend/network
changes, other distros and architectures. Authenticated backend results follow. No production-readiness claim. Local `.env`/`.env.*` and
`*.profile` are ignored; never commit account credentials.

## Authenticated backend verification — 2026-10-21

Real user-authorized account read privately from ignored owner-only `.env`.
Credentials were never printed, supplied as process arguments or copied into the
VM. Successful live bootstrap/account/manifest validation generated two private
worker requests; those were streamed over stdin into the guest and subsequently
deleted from the host. `.env` remains where the user created it.

`tests/authenticated.rs` passes against real AirVPN. `tests/live_linux.py` passes
on Ubuntu GNOME's VM using Comae and Orion, with default desktop transport options:

- Real WireGuard handshake and default HTTPS tunnel verification.
- Public IPv4 exit differs from the VM baseline; public IPv6 works over the tunnel.
- Both advertised AirVPN DNS resolver families answer real queries.
- Forced physical-interface HTTP/HTTPS and TCP/UDP DNS cannot bypass the tunnel.
- Stop first server, retain lock between workers, connect second server successfully.
- Disconnect second server and Recover: tunnel/resolver resources restored,
  ordinary public IPv4 and physical HTTP/TCP+UDP DNS remain blocked.
- Reconnect second server successfully, SIGKILL worker, Recover: protection remains.
- tcpdump on the physical veth observes zero plaintext DNS/DoT or direct HTTPS
  probe packets throughout connections, switching, disconnect and crash recovery.
- Only explicit lock-disable restores baseline public IPv4 and physical DNS access.
- Unrelated nftables table preserved; temporary VM veth/NAT/namespace removed and
  original VM forwarding setting restored by the runner.

Limits: this verifies the actual privileged backend with an internet-connected
isolated network/mount/PID namespace, not the GUI's polkit prompt/automatic retry
transaction. The VM uplink is IPv4-only; tunnel IPv6 is real, while functional
IPv6-underlay bypass prevention is separately covered by local-peer dual-stack
tests. Packet capture checks the exercised probes; it is not an exhaustive claim
about every possible protocol or failure timing. No reboot/suspend/Wayland or
production-readiness claim.

Reproduce from this app's repository on the host, with a built Linux binary and
this source tree mounted in the disposable Lima VM:

```sh
python3 scripts/verify-airvpn.py --vm wave-desktop \
  --guest-binary /home/wave-lab/wave-build/debug/wave-vpn
```

The runner parses only JSON-string credential values in `.env` and checks file
ownership/mode; it does not execute or source that file. The account test is
ignored in ordinary CI. There are no committed private profiles or credentials.

## Account flow and live Network Lock toggle — 2026-10-21

Account presentation now matches the user's compact archived-UI reference:
Credentials / Configured, with Sign out. No Save/Load/Forget keyring controls or
technical storage text. Startup restores and authenticates a saved account;
successful sign-in automatically saves it. Sign-out deletes storage before
clearing the session; failure retains the session and an actionable error.

Verified in GNOME using a private D-Bus/keyring/config session and the authorized
real account: GUI startup loads the item, authenticates, saves it back; clicking
Sign out removes the item. Temporary encrypted collection/config were removed;
the normal desktop keyring was not modified. Verified the visible plaintext
warning with an unavailable D-Bus/keyring. Offline tests cover owner-only
plaintext save/restore/delete, symlink/permissive-file rejection, keyring
selection, migration of an existing plaintext fallback into the keyring and
removal of both storage forms on sign-out. A locked keyring is not treated as
permission to save plaintext.

Network Lock is one toggle, enabled while connected as well as disconnected.
IPC sends bounded enable/disable commands to the existing privileged worker;
no tunnel teardown. State and saved preference change only after firewall
acknowledgment. Pending operations disable duplicate input. Namespace packet
tests verify repeated off/on changes preserve tunnel traffic, restore/block
physical traffic, block DNS when enabled, and preserve the last confirmed
choice across disconnect. Recover now respects an explicitly disabled lock.

Rust all-target tests/Clippy and macOS UI build pass; Linux packet/keyring checks
are rerun for this change. This does not add reboot/Wayland/polkit coverage.

## Tray and one-time networking authorization — 2026-10-21

- Replaced theme-name tray lookup with self-contained antialiased ARGB pixmaps
  at 16/22/24/32/48 pixels. GNOME Shell 46's AppIndicator host visibly renders
  the power badge correctly. Added a matching authored launcher SVG.
- Installer deploys a root-owned socket-activated helper and authorizes the
  invoking desktop UID once. GUI actions use this helper instead of prompting
  for each worker. No password is stored. The optional boot-persistent lock
  remains separate; installing the networking helper does not enable it.
- Installed systemd socket/service on Ubuntu GNOME. Verified uninstall/reinstall,
  helper upgrade via install.sh, service restart and subsequent unprivileged
  recovery without a sudo/polkit prompt. Staged install/uninstall also passes
  and does not modify live systemd units.
- `WAVE_SERVICE_TEST_VM=1 python3 scripts/test-service.py` passes as the authorized
  desktop user: foreign UID rejection, forbidden/oversized actions, malformed
  worker input, incomplete-action timeout, worker streaming with controller stdin
  left open, continued availability and helper/config permissions.
- `sudo env WAVE_TEST_SERVICE=1 python3 tests/linux_network.py <binary>` passes
  the entire namespace packet suite through an isolated socket-activated helper.
  Includes dual-stack tunnel traffic, DNS/bypass protection, live lock toggles,
  EOF, worker SIGTERM, client SIGKILL, worker SIGKILL, switching, concurrency,
  handshake expiry, repeated cycles, resolved/resolvconf cleanup and persistent
  policy compatibility. Boot-only lock-restore is invoked directly as root; it
  is deliberately outside the user-facing helper allowlist.
- All-target locked Rust tests and strict Clippy pass on Linux and macOS.
  Existing macOS `block 0.1.6` future-compatibility warning remains. Android
  unchanged. Real-account backend checks above were not repeated for this change.

Limits: service restart is verified, actual OS reboot is not. Other distributions,
Wayland and complete GUI connection/retry transactions remain unverified.

## Desktop presentation follow-up — 2026-10-21

- Ubuntu Linux: locked binaries build, all-target tests (including ARGB shield
  rendering) and strict Clippy pass. Host Rust tests/build/Clippy also pass;
  the existing macOS `block` future-compatibility warning remains.
- Reused the original authored shield with white wave from the archived SVG;
  connected/stalled/working/idle tray colors retain their existing mapping.
- Server layout now uses available content width for compact mode, consistent
  header/body columns and a scrollbar gutter; sort arrows use inline SVG icons,
  title hover follows label bounds, and Best server has no extra “Now:” label.
- Light dropdown menus follow the app's surface palette; off switches use a
  lighter gray. This is a presentation change, with no VPN behavior changes.
- Synthetic Linux GUI captures checked at 560, 900 and 1100 pixels wide: search
  fits, compact rows retain locations, wide columns align and Score stays inline.
- Switch hover now changes the track in both on/off states: darker blue/gray in
  light mode and brighter blue/gray in dark mode, with a white thumb.
- Sidebar footer separates status and Network Lock with more padding and bottom
  space. Recover has a tooltip explaining DNS/tunnel cleanup and retained lock
  protection, adapted from the archived presentation wording.
- Old-layout reference follow-up: checked the synthetic 900px Linux rendering
  with all six columns and a single-row toolbar. Rows are compact; Server and
  Location share available width, long locations have full-text tooltips, and
  column headings remain above the scroll area. Narrow windows retain compact
  presentation. Recover's explanation is now a short single line and sidebar
  Network Lock text does not wrap.
- Server heading alignment uses the body row's 24px favorite slot, 8px gap and
  14px cell padding. Best server text uses the same left edge. Header hover only
  changes text color, and sorting uses stemless up/down chevrons.
- Score follow-up adds independent synthetic ranking tests: 21% load and 27ms
  yields 48 despite a higher user count; distant/congested servers rank lower.
  Display, sorting and best-server selection call the same scoring function.
  Missing ping, heavy congestion, capacity bonus and warnings are covered.
- Final 900px Linux capture confirms Server/Best server/name left-edge alignment,
  text-only heading hover and stemless Score chevron. Linux binaries build;
  all-target tests and strict Clippy pass on Linux and the macOS host.
- Best server icon alignment follow-up: lightning uses the same 24px icon column
  as stars, followed by the same 8px gap and 14px text inset.
- Linux render confirms lightning/star centers align while Server, Best server
  and names keep their shared text edge. Build, all-target tests and Clippy pass.
- Subsequent layout adjustment centers both icons in the full 46px leading
  column, retaining the name alignment. Hover/selection spans the favorite column;
  the star remains an independent button. Dropdown borders and power-button
  glyph/hover colors have stronger contrast in both themes.
- App package/display version is now 0.1.0 and the bootstrap software identifier
  uses 0.1-dev. Host all-target tests, strict Clippy and locked release build pass.

## Desktop release requirements implementation — 2026-10-21

This section supersedes the earlier feature inventory, without turning local
VM evidence into a production support promise. Android is unchanged.

Implemented:

- Per-family AirVPN HTTPS tunnel-identity checks and a random system-resolver DNS
  challenge/receipt, referenced to pinned official Eddie 2.27.2 Service.cs.
  Overview shows verified/failed/unavailable/not-checked results. Checks repeat
  every 60 seconds; an unavailable check is not a verified connection or proof
  of a leak. Generic/local peers do not acquire provider-verification claims.
- Actual owned nftables query through the authorized helper, with unknown status
  on failure/conflict, no dependence on cached protection JSON, and rejection of
  query replies predating a local networking change. Startup can identify pending
  recovery. GUI explains the ordinary session lock's reboot limit.
- Recovery available before login; a new exclusive worker also repairs Wave's
  leftover journal after a crash. No third-party firewall/DNS state is reset.
  Encrypted account refusals are distinguished from bootstrap/network failures.
- Cancellable helper setup and retries; transient monitor failures back off from
  3 to 120 seconds, reset after a stable session, and remain cancellable while
  waiting. Setup/configuration/recovery errors stop automatic retry. Failed data
  refreshes no longer start a new login attempt on every GUI tick.
- Support report preview/copy/export with check/protection results, tool presence
  and fixed failure categories. No raw logs/accounts/keys/addresses/environment
  enter the report. Export writes exactly the preview, mode 0600, no overwrite.
- Optional local candidate signing and isolated-keyring signature/fingerprint/
  checksum verification. No maintainer private key generated or release published.
  Uninstall now queries actual protection, refusing unknown/enabled state.

Passed:

- macOS and Ubuntu ARM64: all-target Rust tests, strict Clippy, debug binaries.
  New regression tests cover mismatched identity/DNS tokens, foreign/malformed
  firewall state, stale status replies, stopped-session UI, retry cancellation,
  setup error retry refusal, diagnostic contents/permissions/overwrite and clocks.
- Real AirVPN sessions: IPv4/IPv6 tunnel identity and system DNS receipt verified;
  normal tunnel traffic and protected switch/disconnect/crash/recovery passed.
  Wrong system DNS and unavailable provider endpoint refuse Connected and retain
  Network Lock. The final combined capture run passes with protection enabled
  before capture: no exercised physical DNS/DoT/direct-HTTPS probe bypass during
  connections, switching, disconnect, crash, wrong DNS or check-service outage.
  Encrypted sign-in bootstrap also succeeds with deliberately broken DNS and an
  enabled lock, before any tunnel; only synthetic invalid credentials are used
  in the VM, and the decrypted account refusal is correctly classified.
- Expanded namespace suite through the broker: actual status with stale/deleted
  caches, foreign-table rejection without modification, automatic recovery on
  the next connection after SIGKILL, uplink down/up, and 16-second worker pause.
  Pause detection exercises the resume path; it is not a physical suspend test.
  Existing lock toggle/forwarding/routes/DNS backend/crash suites also pass.
- GNOME X11 and **nested GNOME Wayland**: diagnostics, unknown protection and
  connected screens render. X11 actual mouse actions Copy report and Save report
  produce byte-identical JSON with mode 0600. Nested Wayland has no tray host,
  and the app remains visible. This is not a full GDM/PAM Wayland session test.
- Installed helper authorization/UID rejection, input timeout/rejection and
  availability checks; isolated Secret Service save/load/delete tests pass.
- Actual official **Eddie 2.26.2** writes default-, password- and keyring-protected
  v2 profiles; Wave imports all three without modifying source bytes or removing
  the Eddie keyring item. The updated runner creates a private bus/keyring and
  does not use a normal desktop collection. Refreshed official ARM64 APT metadata
  still offers 2.26.2; 2.27.2 runtime package interoperability remains a gate.
- Linux synthetic signing test: valid signature/fingerprint/checksums accepted;
  wrong trusted fingerprint, altered artifact and altered manifest rejected.
  The host Nix GPG could not create its ephemeral test key; the Linux test passed.

Test corrections: capture originally started before the initial lock, including
an intentionally unprotected baseline boundary. The combined runner now enables
and checks the lock before capture. The uplink test also exposed that Linux can
remove the IPv6 physical route on link-down, letting an unbound probe travel
through the encrypted tunnel. Physical leak probes now bind the physical NIC,
with IPv6 underlay restored after link-up; encrypted DNS is not counted as bypass.

Remaining release gates: x86_64 execution, real suspend/resume, full GNOME Wayland
session interactions, complete real-account GUI/polkit transactions, runtime import with an available 2.27.2 package,
maintainer signing fingerprint and working private security contact. The GitHub
repository is private and its private-vulnerability-reporting endpoint returned
404; source changes cannot establish a public contact. No production readiness
or exhaustive leak-proof claim is made. See RELEASE-GATES.md.

Actual reboot attempt: the guest accepted `systemctl reboot`, but Lima/VZ did not
restore guest-agent or SSH connectivity within the bounded test deadline. Host
agent reported guest-agent connection resets and no route to the guest. This is
recorded as a failed environment-level reboot check, not an application pass.
The VM was force-stopped and cold-started to restore the test environment; cold
startup/helper evidence is recorded separately below. Warm reboot and full
reboot policy/order coverage remain release gates.

VM recovery follow-up: offline read-only inspection found the original 6.8.0-134
kernel/initrd alongside a newer installed kernel. Direct boot of the known test
kernel, preserving the root disk, plus masking the startup network wait restored
SSH, DNS, the guest agent and GNOME X11/viewer services. Original Lima YAML and
EFI variables were restored; direct-boot files/arguments are local VM recovery
artifacts, not application dependencies or committed files. The original EFI
boot failure's exact cause is not established. Actual cold startup now confirms
that the installed authorized helper works without another authorization and
reports protection off, with no active worker/recovery journal. This workaround
is not proof of standard EFI boot, boot-persistent lock ordering or suspend.

Final negative-provider run also passes a partial IPv6 endpoint outage: IPv4
identity verifies, IPv6 is unavailable, Connected is refused, and the lock/DNS
block remain. Outgoing capture is identified by the isolated NIC's Ethernet
source, independent of source IP, so incoming baseline HTTPS retransmissions are
not mislabeled as leaks. The final run reported total=0, outgoing=0, incoming
baseline HTTPS=0. An earlier expanded run had three unclassified capture packets;
its direction was not retained, so no cause is asserted for that run. The latest
explicit direction-aware capture and the prior full combined run both pass.
A focused broker lifecycle rerun after the monitor clock adjustment also passes;
the monitor excludes its own bounded verification work from resume detection.

The bounded actual reboot check then **passes on the recovered VM**: new kernel
boot ID, authorized helper available without a new password prompt, and actual
protection off with no active worker or recovery journal. This uses the local
known-kernel/network-wait workaround above; it does not validate the original
EFI startup path, an enabled persistent-lock policy, or physical suspend.
Final UI handling retains specific DNS/identity/per-family check failure messages
and results after worker exit instead of replacing them with a generic error.

Final all-target tests and strict Clippy pass again on both macOS and Ubuntu
ARM64 after the last error-handling changes. Missing account devices now request
account configuration rather than network recovery; missing getent is detected
before configuration and does not enter transient network retry. Final Linux
binaries are installed in the recovered VM. Android worktree is unchanged.

## Round 3 evidence audit — 2026-10-22

The prior round's appended broad runtime claims were withdrawn pending command
evidence. Synthetic `WAVE_PREVIEW` scenes, an unlocked synthetic Secret Service
collection, automatic desktop login, and environment provisioning do not prove
authenticated GUI transactions or PAM keyring unlock. The original delegated
thread was unavailable to that child; the parent subsequently inspected its
durable command timeline.

Starting source: `5b8360472f41be43b0376b7403eab154d41225c8` plus uncommitted
changes. Existing Ubuntu lab VM CLI/helper SHA-256 at audit start:
`63e32f72cc8c31bcdd1c9b48b53a1e2056fd9302fb871d6d678f69b58f0bfc65`.
This identifies an inherited binary, not a newly accepted candidate.

Open gates remain: actual top-level GNOME Wayland account/sign-in, connect,
cancel, retry, switch, status and tray interactions; manual PAM/keyring login;
installed-helper EFI reboot with session and enabled persistent-lock ordering;
install/upgrade/uninstall and dependency/firewall/state conflicts; latest
official Eddie package discovery and import variants; architecture-specific
build/test evidence; and physical Linux suspend/resume with safe recovery.
Authenticated backend or namespace tests alone cannot close the GUI gates.
Release publication, signature identity, and confirmed delivery to the private
security reporting inbox remain coordinated by the parent; this audit does not
claim those gates complete.

## Round 4 release verification — 2026-10-23 UTC

Candidate evaluation on x86_64 Ubuntu 24.04 GNOME Wayland (guest 113) and
ARM64 lab guests (`wave-debian-xfce`, `wave-fedora-kde`).

Candidate source baseline: `5b8360472f41be43b0376b7403eab154d41225c8` with
peer-authorization-before-slot-reservation hardening in `src/service.rs`,
regression test strengthening in `scripts/test-service.py`, and subprocess
SIGINT inheritance reset in `scripts/test-eddie-import.py`.

Candidate binary hashes (x86_64 release build, Ubuntu lab VM):
- `wave-vpn`: `684e907e228945d6380d7dc3c8bbd945323fbf02c309940b2eb3e7b52e96b182`
- `wave-vpn-gui`: `e11b7d325d64dff21eac146acc4af99476039ad0512f3430c16c88a1ac0cbcdd`
- `wave-vpn-helper`: `684e907e228945d6380d7dc3c8bbd945323fbf02c309940b2eb3e7b52e96b182`

Verified gates:
1. Privileged service broker regression (`test-service.py`):
   - Unprivileged recovery without sudo/polkit prompt: PASS.
   - Foreign peer credentials rejection via SO_PEERCRED before worker slot allocation: PASS.
   - Saturated slot resistance: 32 concurrent unauthorized peers rejected with all 8 authorized slots occupied; authorized progress continues under sustained foreign load: PASS.
   - Dynamic authorization reload: Revoked UID rejected at socket admission; restored UID takes effect without broker restart: PASS.
   - Oversized input, malformed actions, and incomplete request timeouts: PASS.
2. Real EFI guest reboot and boot-persistent lock ordering:
   - Verified on Ubuntu lab VM (`ovmf` EFI firmware).
   - `wave-vpn-lock.service` started at 01:10:47 UTC, exited with status 0 at 01:10:48 UTC.
   - Monotonic journal confirms `network-pre.target` reached at 01:10:53 UTC (5s after lock service completed).
   - Boot-persistent `table inet wave_vpn` rules were present after boot, and the
     lock service completed before `network-pre.target`. This is service-ordering
     evidence; no boot-time packet capture established every interface transition.
   - Out-of-band recovery and clean uninstall confirmed via QEMU Guest Agent (`lock-disable` and `lock-uninstall`), removing unit files and restoring normal networking.
3. Real GNOME Wayland GUI and StatusNotifierItem tray lifecycle:
   - Real top-level Wayland session on Ubuntu lab VM.
   - Real AirVPN account connection to Khambalia, with provider IPv4, IPv6 and DNS
     challenge checks displayed as verified. These GUI/tray runs preceded final
     release-binary installation; do not assign the final release hashes to them.
   - Server switching (Guniibuu -> Khambalia) verified.
   - Worker SIGKILL failure recovery: GUI correctly reported connection failure, retained Network Lock protection, and restored network state via Recover.
   - StatusNotifierItem DBus object (`org.kde.StatusNotifierItem-20783-1`): Title dynamically updated (`Wave VPN — Connected` / `Wave VPN — Disconnected`), MenuBar layout verified, Show Wave VPN activated, Disconnect via tray disconnected VPN tunnel and retained lock, and Quit via tray cleanly terminated GUI process.
4. Namespace network test suites (`tests/linux_network.py`):
   - Ubuntu lab VM (Ubuntu 24.04 x86_64): Direct mode (19/19) + Service mode (20/20) PASS.
   - `wave-debian-xfce` (Debian 13 ARM64): Direct mode (19/19) + Service mode (20/20) PASS.
   - `wave-fedora-kde` (Fedora 44 ARM64): saved service run exited 0; saved direct
     run exited 1 when its isolated resolver did not become ready. The previous
     broad direct-suite pass claim is withdrawn pending a bounded rerun.
5. Official Eddie 2.27.2 profile import (`scripts/test-eddie-import.py`):
   - Verified on Ubuntu lab VM with official installed Eddie 2.27.2.
   - Successfully wrote and imported v2n (default XML), v2p (password-encrypted), and v2s (Secret Service / secret-tool protected) profiles without modifying source (`EDDIE_EXIT=0`).
6. Service and lock install / uninstall lifecycles:
   - `service-install` / `service-uninstall` verified.
   - `lock-install` / `lock-uninstall` verified.
   - Conflicting table and foreign table detection verified.
7. Local cargo check / test / clippy and release verification:
   - `cargo fmt --check`: PASS.
   - `cargo clippy --all-targets`: PASS.
   - `cargo test`: PASS; opt-in live/private-bus tests remain separately tracked.
   - `scripts/test-release.py`: Detached signature, SHA256SUMS manifest, fingerprint validation, and tamper rejection PASS.

Residual open gates (parent review):
- Final-release-binary authenticated GUI/tray transactions, complete account/error
  matrix and manual PAM/keyring login have not all been demonstrated.
- Full install/upgrade/uninstall and missing-dependency/state-conflict coverage
  across the declared support matrix remain separate from helper-unit lifecycle.
- Source tags, final production signing/verification, source/download publication
  and security-inbox delivery remain outstanding.
- Safe physical suspend/resume: OPEN. Requires real bare-metal Linux hardware where physical ACPI suspend/resume can be tested safely without disrupting shared hypervisor infrastructure.


## Parent review — 2026-10-23 UTC

The parent independently matched all 26 application source/toolchain-input files
between the local reviewed tree and Ubuntu lab VM. Installed CLI/helper hashes match
`684e907e228945d6380d7dc3c8bbd945323fbf02c309940b2eb3e7b52e96b182`;
GUI hash matches `e11b7d325d64dff21eac146acc4af99476039ad0512f3430c16c88a1ac0cbcdd`.
The installed helper regression suite passed again, including authenticated
progress during sustained foreign traffic and live authorization revocation.
Current helper status reports protection off, persistence off, no recovery journal
and no active worker; the socket is listening and persistent-lock unit is removed.
The test-introduced GNOME lock/idle/mouse overrides were reset to distribution
defaults; exact prior override values were not retained across the guest reboot.

Debian ARM64 saved packet log SHA-256:
`58346031e4746370b7c3926b8155af6717d083983abf70c617489c92233a3b13`;
both direct and broker exit markers are zero. Fedora's saved log SHA-256
`3dea6192d8bffc759b82052d0e5d43242862454793cd9a0adfe5c186833f847c`
contains direct exit 1 and broker exit 0. Parent reproduced the isolated-resolver
startup failure. A diagnostic-only run with longer startup allowance passed and
measured approximately 30 seconds to readiness. The test now bounds overall
resolver readiness and each client call by a shared 45-second deadline; it still
fails if the resolver exits or never becomes ready. Failure logs are retained;
this startup diagnosis is not a blanket Fedora support claim.


The final bounded-readiness Fedora direct rerun still exited 1 at isolated
resolver startup (16 PASS lines before that stage). Log SHA-256:
`47f22f29be9ac99097d29b86844df06fa4baea9e707c35e7c44e75f6927a9c24`.
The longer deadline alone does not establish a fix or the exact cause. Fedora
broker-mode evidence remains separate; Fedora direct qualification is open.
The helper and Ubuntu support-target evidence are not relabeled by this result.

### Fedora resolver client correction — 2026-10-23 UTC

Further isolated diagnosis reproduced a client failure while systemd-resolved
remained alive and answered D-Bus requests. Against the same running daemon,
three comparisons returned SIGPIPE (`-13`) for `resolvectl status` with both
streams redirected to `/dev/null`; explicit `--no-pager` returned zero in every
comparison. Defunct `less` children were observed. This identifies a pager-related
client failure in the reproduced harness. The historical failures discarded
client status/stderr, so their individual causes cannot be proved retrospectively.
The earlier 30-second retry timing did not establish daemon startup duration.

The test now disables paging for resolver queries, retains client error details,
bounds each probe to two seconds within the existing 45-second overall deadline,
and terminates/reaps its private resolver in `finally`. Packet and DNS assertions
are unchanged. Corrected full direct and broker suites exited zero, with 19 and
21 literal PASS lines respectively. Environment: Fedora 44 ARM64 KDE Lima guest,
kernel `6.19.10-300.fc44.aarch64`, systemd `259.5-1.fc44.aarch64`.

Reviewed evidence SHA-256:

- Direct log: `14d8ddc5b78f731a750f672eb73889682baf7276ccea04aa6211cb9c6a02bdf9`.
- Broker log: `b5173fe11573891d0965d3e315e088160fb7bacd81150c87c7c15bcef9b3eb23`.
- Same-daemon comparison: `a49a6317e6c7b6ed18d3bf6dbba34c65deb9c2aa4d1915bccb0ed126e8bb33aa`.
- Corrected test source: `328bf7efb4b02775c1d2a5fb36673bd2e8f251227d62772be663fd9749137f11`.
- Tested debug CLI: `1bcfc814f13965da57bfb6f5f569b0baa4e4e8ba1265f4146737f82b85a83ee8`.

Private review evidence is `/tmp/wave-fedora-review`; guest commands run
`sudo env TERM=xterm-256color python3 tests/linux_network.py target/debug/wave-vpn`
from its source checkout, adding `WAVE_TEST_SERVICE=1` for broker mode. No test
namespaces or processes remained after cleanup. Parent reviewed the patch,
comparison logs and evidence hashes. Local locked Rust tests and strict clippy
passed. This closes the reproduced harness failure, not final-release GUI,
installed-service/PAM-keyring, physical suspend or general Fedora qualification.


### Bounded final-release GUI account/error check — 2026-10-23 UTC

The installed VM113 GUI was checked through `/proc/<pid>/exe`: normal Wayland
PID 7319, isolated authenticated PID 11460, restart-restored PID 12457; all matched
release GUI SHA-256 `e11b7d325d64dff21eac146acc4af99476039ad0512f3430c16c88a1ac0cbcdd`.
Installed CLI/helper were rechecked against the previously recorded release hash.

Passed: actual final GUI synthetic rejected-account message; distinct unreachable
bootstrap/recovery message under a temporary outbound rejection rule; GUI Recover
before authenticated sign-in retained protection (guest-agent status confirmed
no worker or recovery journal); actual authorized sign-in/save and automatic
restart restore in an isolated encrypted Secret Service/D-Bus session displayed
on the real GNOME Wayland desktop. This isolated keyring was explicitly unlocked
for testing; it is not PAM/login-keyring evidence. No credentials were printed or
supplied in process arguments.

GUI Connect started a real worker, but a final Connected/provider-check display
was not captured. Final-release connect/cancel/switch/retry/stop, tray lifecycle,
GUI sign-out, and manual PAM/keyring login remain unqualified. Automatic GDM login
left the normal login collection locked; session unlock via loginctl does not
close that gate. Work stopped at the parent's bounded wrap-up instruction.

Cleanup confirmed protection/persistence off, no worker/recovery state/tunnel,
active service socket, persistent-lock unit absent, temporary firewall table and
private guest collection/config/logs removed. No GNOME settings were written;
before/after desktop dconf snapshots matched. Normal app netlock preference was
explicitly left false; its original serialized value was not captured, so exact
app-preference byte restoration is not claimed. No app source changes or new
source-failure conclusions. Private filtered report and capture hashes:
`/tmp/wave-linux-final-gui/report.md` and `evidence-sha256.txt`.

## 0.1.2 packaging and installer — 2026-11-05

Scope: the new `.deb`, `.rpm` and Arch packages, the self-contained tarball and
its `install.sh`, and package/uninstall teardown. Networking code is unchanged
from 0.1.1. The artifacts below are a local unsigned candidate built by
`scripts/release.sh` in an `ubuntu:24.04` container from a local `v0.1.2` tag;
the CI release rebuilds them, so these hashes identify the tested candidate only.

```text
4af62cd12b584ac20f06d682084b082ac982f630cfb4c265281ebae468165762  wave-vpn_0.1.2-1_amd64.deb
2855a5349b2c84e75c224ba0380127b2b532ea5d921063c1f1eddf9e3c2348b4  wave-vpn-0.1.2-1.x86_64.rpm
76592279cbc815461193d31f4656787cbe2be1971662ab4e4e29ef4d0fde86b8  wave-vpn-0.1.2-1-x86_64.pkg.tar.zst
18dfc557cdaf38e3878051ac507f69820d54dd2e1c35fa0aa9f292aba37bbd00  wave-vpn-0.1.2-x86_64-unknown-linux-gnu.tar.gz
```

**Ubuntu 24.04 GNOME Wayland VM (lab VM 113):**

- Tarball `./install.sh` run as the desktop user over a pre-0.1.0 `/usr/local`
  install: asked for sudo itself, replaced binaries, refreshed the helper copy,
  kept the authorized UID, removed stale flat docs. Unprivileged
  `service-client network-status` answered without a password.
- With `wireguard-tools` and `nftables` removed, the installer named them and
  installed them after "y"; declining `libsecret-tools` installed the app with a
  warning. The first `apt-get` hit unattended-upgrades' dpkg lock, so the
  installer now waits up to five minutes for it.
- `sudo wave-vpn-uninstall` and `apt remove wave-vpn`, each with Network Lock
  enabled through the helper (LAN allowed): firewall table, helper, units and
  `/etc/wave-vpn` removed, internet reachable, `~/.config/wave-vpn` kept.
- `apt install ./wave-vpn_0.1.2-1_amd64.deb` pulled in `wireguard-tools`; the
  post-install hook authorized the sudo user's helper; `--reinstall` kept it.
- `gtk-launch wave-vpn` found the desktop entry for both installs and the GUI
  stayed running. QEMU could not capture the VirGL display, so no screenshot.

**CachyOS physical laptop, niri Wayland:**

- `pacman -U` installed with the hook authorizing the sudo user's helper;
  reinstall (`post_upgrade`) kept it; `pacman -R` ran the teardown (Arch passes
  the version, not "remove") and removed helper, units and `/etc/wave-vpn`.
- Tarball install and `sudo wave-vpn-uninstall` on a clean system; the tarball
  installer now refuses to run over an installed package.
- The packaged GUI launched from its desktop entry, rendered, and reported
  Wayland `app_id` `wave-vpn` (previously empty, so docks showed no app icon).
- Network Lock was not enabled on this laptop to avoid cutting its connection;
  lock teardown is the same binary and hook verified on Ubuntu above.

**Fedora 45 and openSUSE Tumbleweed containers:** `dnf`/`zypper` resolved the
`.rpm` dependencies (`(iproute or iproute2)`, `/usr/bin/secret-tool`) and `rpm -e`
removed it. The first candidate depended on `(libsecret or libsecret-tools)`,
which openSUSE satisfied with the library alone and no `secret-tool`; fixed. A
rootless container can't run `nft`, which showed removal failing when Wave had
set nothing up; teardown now runs only when Wave state exists.

**Network Lock default:** on the CachyOS laptop, an enabled lock (the old default,
kept after disconnect or a failed connect) cut the maintainer's internet while no
tunnel was up. 0.1.2 makes the lock opt-in. The isolated network suite and the
authenticated live suite now request the lock explicitly instead of relying on
the default; the full isolated suite passed on the Ubuntu VM with an
Ubuntu-built debug binary of that change.

**Published v0.1.2 (CI-built, signed):** the first tag run built and signed every
artifact, but `verify-release.sh` accepted only `.tar.gz` names and stopped the
workflow before publishing; it now also accepts `.deb`, `.rpm` and
`.pkg.tar.zst`, and a local signed `release.sh` run with a throwaway key passed.
The tag, never used by a release, was moved to that fix. The published assets
verified against fingerprint `414018C0BBB8C0A70A80BF570AA32CCB2C0B3951`; the
published Arch package reinstalled on the CachyOS laptop (helper refreshed to
the published binary) and the published `.deb` installed on the Ubuntu VM with
the helper active and the GUI launching from its desktop entry.

```text
f05774eb815d9a5d8640dca35636664da12d68fb2b20738219885aed5ba025ab  wave-vpn_0.1.2-1_amd64.deb
360d96be596492410be649cfc2588d894e86e7a5382a6d5dea48ff58cd917df3  wave-vpn-0.1.2-1.x86_64.rpm
220632d4e4b908c6e9a76ebc8cb05e0ffc85f671ef2b905c8c05cb53643bcaca  wave-vpn-0.1.2-1-x86_64.pkg.tar.zst
284778adac15070e38f6edf5985f89bde707c6128b9aad9c27d72faa3effcb29  wave-vpn-0.1.2-x86_64-unknown-linux-gnu.tar.gz
```

Not covered: GUI connect with these packages, a real Fedora or openSUSE desktop,
and an upgrade between two signed package releases.
