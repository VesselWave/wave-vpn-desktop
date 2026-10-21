# Verification record

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
