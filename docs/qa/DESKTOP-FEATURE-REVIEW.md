# Desktop feature review — 2026-10-21

Recommendation: finish a dependable, small Linux WireGuard client before adding
advanced settings. The user authorized implementation of all five must-finish items after this
review. Advanced features below remain outside the implementation scope. The accepted surface in [PARITY.md](../PARITY.md) remains in force.
Android is outside this review.

## Comparison baseline

- Archived Wave desktop: inspected `old/linux/README.md`, its option registry,
  GUI settings/preferences and historical Eddie reference. These identify behavior
  and possible expectations; their presence does not establish correctness or
  permission to reuse code. No archived implementation was copied.
- Current Wave: inspected controllers, views, options, import, diagnostics,
  helper and Linux verification code, together with the verification record.
- Latest official Eddie desktop: **2.27.2**, released **2026-09-14**, according to
  the [official changelog](https://eddie.website/changelog/?format=debian&hidden=yes&software=client).
  GitHub master checked on the review date is
  `67e213bfeae270f1f26c62f454f9dfadd5342df9`; the existing pinned local reference
  matches it and declares 2.27.2 / protocol version 304. There is no source-pin
  update required by this review. Wave's live bootstrap uses version 295; do not
  change that solely to match a release number without interoperability evidence.
- Official files inspected: `Lib.Core/Constants.cs`,
  `Lib.Core/Providers/Service.cs`, and `Lib.Forms/Forms/Settings.Designer.cs`.
  The archived Eddie reference describes 2.24.6 and is not the current authority.
  Existing installed-Eddie import evidence is for **2.26.2**, not 2.27.2.

## Already present: retain and finish verification

These are not missing features to reimplement:

- Account sign-in/out, automatic Secret Service storage/restore, explicit warning
  for the owner-only plaintext fallback, and read-only Eddie credential import.
- Device/key selection when the account has multiple devices.
- Best-server, country and explicit server selection; search, favorites, latency,
  load/users and shared ranking for display/sort/automatic choice. Exact Eddie
  scoring parity is unnecessary.
- Connect/disconnect, server switching, automatic retry and handshake monitoring.
  Current retry uses a fixed configured delay; network-aware backoff is not yet
  established by the verification record.
- IPv4/IPv6 tunnel routing, automatic DNS backend selection/restoration, Network
  Lock retained after disconnect/Recover/crash, live lock toggle and optional LAN.
- One-time helper installation/authorization, root-owned helper and recovery
  state, fixed actions and bounded helper output. These are already implemented;
  latest Eddie hardening is a source of additional failure cases to check.
- Tray, logs, traffic statistics, appearance/units, saved preferences and periodic
  account/server refresh. There is also a small CLI diagnostic export.

Authenticated backend and isolated packet checks already pass on the documented
Ubuntu VM. That evidence does not establish every interactive or OS lifecycle
workflow. See [VERIFICATION.md](../VERIFICATION.md) for the exact limits.

## Must finish before a public desktop release

1. **Connection and DNS verification inside the app.** Current
   `src/platform/linux.rs::verify_tunnel` checks public routes and, by default,
   performs a tunnel-bound HTTPS request to airvpn.org. It does not verify the
   expected exit identity or perform Eddie's provider DNS challenge. Add a small
   verified/failed/unknown result for tunnel internet and DNS, with actionable
   errors and protection retained on failure. Use the pinned licensed provider
   behavior as the reference. A handshake or successful website request alone
   must not be presented as proof of complete leak protection. Test intentional
   wrong DNS, no internet, partial IPv4/IPv6 failure and provider-check outage.

2. **Recovery through real desktop lifecycle events.** Exercise GUI connect,
   cancel, switch, automatic retry, helper setup cancellation/failure, keyring
   locked/unavailable, worker/service restart and reopening after a crash. Add
   or fix handling where needed for suspend/resume, Wi-Fi/Ethernet changes and
   losing/regaining the uplink; verify lock and DNS throughout. Make prolonged
   retries bounded in rate and cancellable; retry transient failures while
   explaining account/device/configuration failures that require user action.
   This is principally a coverage/correctness gap, not a claim that all these
   mechanisms are absent.

3. **Clear protection and recovery state.** Keep connection and firewall state
   separately visible, including stalled/reconnecting and disconnected-but-locked.
   Establish actual state on startup and after helper/state-file loss; avoid
   presenting a saved preference or stale status file as confirmed protection.
   Explain that session protection ends at reboot unless the optional persistent
   lock is installed. Keep an obvious explicit unlock and safe Recover path.

4. **Useful private support diagnostics.** Extend the existing allowlisted CLI
   snapshot (currently app version, OS/architecture and explanatory text) with
   safe connection/protection/check results, dependency/backend availability and
   bounded redacted failure context. Add GUI preview and save/copy. Exclude
   credentials, keys and raw profiles; make address/account/log handling explicit.
   No automatic upload. Existing logs and the metadata snapshot do not yet meet
   this workflow.

5. **A tested installation/release boundary.** Pick and publish a narrow initial
   Linux/systemd support matrix; verify Ubuntu x86_64 as well as the existing
   ARM64 VM evidence, plus GNOME Wayland and tray-unavailable behavior. Test real
   reboot, helper startup ordering, upgrade/uninstall/recovery and coexistence
   with existing firewall rules. Check latest Eddie failure cases: unavailable
   tooling, conflicting/owned firewall tables, missing state directories and
   reconnect after a session ends. Re-run read-only import against Eddie 2.27.2
   with default/password/keyring protection. Ship checksums/signature verification
   and exact source tags, and configure a usable private security contact.
   SECURITY.md, SUPPORT.md and CI definitions already exist; finish their release
   requirements rather than listing them as wholly absent.

## Useful next, after the release gates

- A simple persistent Network Lock installation/status workflow, only after real
  boot ordering and recovery are verified. CLI support already exists. This is
  distinct from the networking helper and from autoconnect.
- Account expiry/device errors and clearer freshness of server data. Prefer
  compact explanatory messages to another settings tab.
- Optional launch at desktop login, then connect on launch if wanted. Treat these
  separately from a root service connecting before login; neither is required
  for the currently accepted manual-connect scope.
- Distribution packages and update notifications after one supported installation
  path is reliable. Do not block a first release on every package format.

## Out of scope for now

- **Boot autoconnect and full headless daemon/CLI parity.** Archived Wave offered
  these, but they add credential access and service-ordering requirements beyond
  the accepted desktop flow. Preserve existing safe CLI functions without making
  every GUI feature a daemon API.
- **Split tunneling, custom routes, entry-interface selection, firewall IP
  exceptions, forwarding/NAT and gateway/hotspot operation.** Archived options
  and some current worker compatibility paths exist; they are not a reason to
  restore the advanced GUI. Each broadens routing and leak-test combinations.
- **Custom DNS/backends, MTU/keepalive/timeout/port controls and scoring sliders.**
  Keep automatic safe defaults. If real networks demonstrate a need, consider
  automatic endpoint fallback or one narrowly tested option first.
- **OpenVPN, SSH/SSL/Tor/proxy transports, obfuscation and multi-provider profile
  management.** Eddie serves these use cases; WireGuard-only is an intentional
  product limit. UDP-blocked networks remain unsupported until scope changes.
- **AirVPN account-side port forwarding, DDNS/blocklists, DuckDNS callbacks and
  event scripts.** Archived DDNS/options are a separate automation surface.
  Ordinary use of a port configured on AirVPN's website is different from adding
  an in-app account management feature; do not promise inbound-port support
  without verifying the current firewall policy.
- **Full Eddie settings import.** Import credentials and the supported preference
  only; explain omissions. Never silently import routing/firewall exceptions,
  startup behavior or scripts.
- **Windows/macOS VPN backends, non-systemd Linux, multiple firewall engines,
  every desktop environment and automatic self-updating.** Expand the support
  matrix deliberately after the initial Linux target is proven.

The first implementation iteration should cover lifecycle correctness and
connection/DNS checks, then diagnostics and release verification. Keep the current
two settings tabs; most of this work needs better behavior and evidence, not more
controls.

## Implementation follow-up

Provider identity/DNS challenges, actual firewall status reconciliation, recovery
before a new connection, cancellable setup/backoff, support-report preview/copy/
export and signed-candidate verification are now implemented. Verification and
remaining release gates are tracked in [VERIFICATION.md](../VERIFICATION.md) and
[RELEASE-GATES.md](../RELEASE-GATES.md). Implementation does not itself complete
physical suspend, architecture coverage, release signing or a public contact.
