# Provenance

## In short

- **License:** GPL-3.0-or-later. Keep the notices below when you distribute source or binaries.
- **Protocol:** written new in Kotlin and Rust from AirVPN's official, GPL-licensed Eddie and Eddie for Android sources, pinned to exact commits below. This isn't a formal clean-room process or a third-party legal audit.
- **Copied data:** only the pinned Eddie provider definition (`protocol-data/AirVPN.json`) and a public example manifest. No account data.
- **Reused design:** the maintainer's own authored UI (Compose screens and theme, desktop icons) and the Geist fonts (SIL OFL) and Lucide-derived icons (ISC). Everything else, including storage, tunnels and tests, was written new.
- **Excluded:** the archived upstream desktop project has no license, so none of its backend, tests, helper, packaging or inherited GUI code is used.
- **Crypto honesty:** AirVPN's sign-in wire format is RSA plus AES-CBC without a separate MAC, matching Eddie. It isn't authenticated encryption, and we don't claim it is.
- **Linux networking:** uses the distribution's `wg`, `ip` and `nft` tools, checked against Eddie's Linux sources; no source text copied.

The sections below are the detailed record.

## Protocol references

New Kotlin and Rust implementations were written against these official sources:

- [AirVPN/Eddie](https://github.com/AirVPN/Eddie), commit
  `67e213bfeae270f1f26c62f454f9dfadd5342df9`:
  `src/Lib.Core/Providers/Service.cs` (`FetchUrl`, `AssocToUtf8Bytes`, account,
  manifest and WireGuard field mapping), `resources/providers/AirVPN.json`.
- [AirVPN/EddieAndroid](https://gitlab.com/AirVPN/EddieAndroid), commit
  `1376541c05f6d8b2ca8b07ffb790b2c95615cf9c`:
  `SupportTools.java` (document version 295, request envelope, bandwidth units),
  `AirVPNManifest.java`, `AirVPNUser.java`, `AirVPNServer.java`.

Eddie copyright (C) 2014-2026 AirVPN (support@airvpn.org), GPLv3 or later.
Protocol implementations are newly written adaptations/reference-based work;
this is not a claim of a formal clean-room process or a third-party legal audit.

`protocol-data/AirVPN.json` is copied from the pinned Eddie provider definition.
`protocol-data/fixtures/eddie-manifest.xml` is copied from the pinned Eddie Android
`app/src/main/assets/manifest.xml`. It contains public example server metadata,
not account credentials. The bundled manifest omits live server addresses.

The wire format is RSA PKCS#1 v1.5 wrapping a random AES-256 key/IV and
AES-CBC/PKCS padding for data; associations encode base64 names and values.
Bootstrap HTTP is intentional for direct-IP access, matching Eddie. CBC is not
AEAD and has no independent MAC here; we do not claim otherwise.

Protocol scoring retains the pinned Eddie Android base/load/users calculation.
Desktop displayed scores, score sorting and best-server selection share a new
implementation based on the pinned Eddie `Lib.Core/ConnectionInfo.cs` speed
terms. Wave's desktop policy reduces the user-count term by 1000 and adds a
quadratic congestion cost above 60% load, matching the intended old app policy
illustrated by the supplied screenshots. Measured TCP latency contributes to the
score; unmeasured latency contributes zero until available. Retry penalties
remain a separate selection safeguard. This policy is not exact Eddie parity.
The archived scoring code was inspected to identify behavioral differences; no
backend code or tests were copied. Desktop implementation and synthetic tests
were written anew. Android scoring is unchanged.

## Authored presentation reused

The user identifies VesselWave's Android UI and desktop redesign as their work.
The archived desktop design contribution is recorded at base `8a24d924089513d368a7031510d79a055cf85ca5`,
head `2842026e4420a1a34ea85abd65a563bc5c7e0e7e`. The contribution modifies
existing GUI files, so it is not treated as permission to copy every file it touches.

- Android: authored Compose screens/components/theme/icons from the archived
  Android repository at `cd2538c`, plus UI resources and ordinary Gradle tooling.
  `AppViewModel.kt`, formatting, application wiring, storage, tunnel management,
  protocol implementation and tests were written anew.
- Desktop: `icons.rs` (a new file added by VesselWave), Geist font files and their
  notices. Controller, views and theme implementation were written anew using
  the authored visual design. No archived IPC/helper/state-machine files copied.
- Geist/Geist Mono: SIL Open Font License, notices in Android `licenses/` and
  Linux `resources/fonts/OFL.txt`.
- Lucide-derived icons: ISC, notices in Android `licenses/` and Linux
  `resources/LUCIDE-LICENSE.txt`.
- Gradle wrapper: Gradle's standard wrapper/tooling distribution, Apache 2.0.

The absence of a license on the archived upstream desktop project is the reason its backend,
tests, helper, packaging and inherited GUI application code are excluded.
Neither old repository history nor a dependency on it is included in this tree.

## Linux transport references

The fresh Linux backend uses the distribution's `wg`, `ip` and `nft` tools;
it does not require Hummingbird or `wg-quick`. The shared protocol and transport
request API have no dependency on these tools. Platform effects are confined to
`src/platform/linux.rs`; no macOS/Windows VPN backend is implemented yet.

Behavior was checked against the pinned Eddie sources above:
`Lib.Core/ConnectionTypes/WireGuard.cs`, `Lib.Platform.Linux/Platform.cs`,
`Lib.Platform.Linux/NetworkLockNftables.cs`, and
`App.CLI.Linux.Elevated/src/impl.cpp`. Eddie itself uses kernel UAPI on Linux;
Wave currently invokes `wg` instead. No source text was copied from those files.

Policy routing and fwmark behavior were also checked against upstream
[wireguard-tools wg-quick](https://git.zx2c4.com/wireguard-tools/tree/src/wg-quick/linux.bash),
blob `18e266c952b466e8797c73fd1aa4f2187f7ec658`, and
[wg(8)](https://git.zx2c4.com/wireguard-tools/tree/src/man/wg.8),
blob `a0fc04c04cf19d64740d1da8edf4055d40cce253`.
These are behavioral references; their GPL-2.0-only source is not copied or bundled.

Linux tray uses the public ksni StatusNotifier API, with new menu/state wiring.
All new controllers, policies, views and tests are independent implementations.

[AirVPN's third-party forum](https://airvpn.org/forums/forum/49-third-party-software/)
provides community integration context, not a protocol specification or a license
grant for projects linked there.

## Repository separation

This app has its own Git history and remote. Protocol reference data and fixtures
are included locally under `protocol-data/`; no sibling checkout is required.
The parent workspace's community checklist coordinates both applications.

## Focused Eddie import

Read-only profile import is newly implemented from the same pinned Eddie commit
`67e213bfeae270f1f26c62f454f9dfadd5342df9`: `Lib.Core/Storage.cs`
(v1/v2 envelope, profile ID, default password and XML options),
`Lib.Core/Crypto/Manager.cs` and its AES implementation (authenticated encrypted
payload), and `Lib.Platform.Linux/Platform.cs` (`OsCredentialSystemRead`, the
Secret Service attribute `Eddie Profile`). Keyring import only reads that item;
it never writes or deletes Eddie items. Only credentials and last-server
preference are imported into the reduced product surface. Synthetic vectors use
Python hashlib/hmac and OpenSSL, authored in scripts/generate-import-fixtures.py.
Runtime interoperability uses the official signed-repository Eddie UI 2.26.2
package in the Ubuntu GNOME development VM, separately installed and not bundled.

## Tray assets and networking helper

`resources/wave-vpn.svg` reuses the authored shield/wave artwork from
`old/linux/resources/airvpn.svg`; its two paths match the already carried-forward
Android launcher foreground. `resources/wave-vpn-tray.svg` uses those same paths
with a transparent background and a tighter view box. This reuse falls under the
authored Android presentation assets recorded above, GPL-3.0-or-later.
`src/tray_icon.rs` independently renders this SVG using resvg (MPL-2.0), already
used by Iced, and converts its output to StatusNotifier ARGB. StatusNotifier tray icons carry their own ARGB pixel data and
do not depend on an icon theme name.

`src/service.rs` is an independently authored systemd socket broker, using
SO_PEERCRED UID authorization and a fixed action allowlist to launch the existing
validated worker. The archived README was inspected only to distinguish its
networking helper from its optional boot-persistent lock. No archived backend,
tests or packaging were copied.

## Desktop verification and recovery review — 2026-10-21

The new provider exit/DNS checks in `src/checks.rs` and the Linux monitor were
independently implemented from the already pinned Eddie commit above,
`Lib.Core/Providers/Service.cs::OnVpnEstablished` (tunnel IP identity, pinned
exit HTTPS endpoints and random DNS challenge receipt). Public manifest check
metadata and exit addresses are now parsed. Only the pinned AirVPN check domain
and DNS-query template are accepted; no scripts or arbitrary check URLs are
accepted by the privileged worker. No additional upstream code was copied.

AirVPN staff reports informed recovery requirements, not implementation reuse:
[stale VPN DNS after dirty shutdown](https://airvpn.org/forums/topic/56835-network-lock-bugged-out/)
and [Ubuntu DNS after reconnect](https://airvpn.org/forums/topic/63864-dns-leak-after-automatic-switch-to-another-server-when-internet-disconnects/).
These support possible causes, not a diagnosis of the user's prior incident.
Actual firewall queries, bounded retry, diagnostics, release verification and
new regression tooling are newly authored. Archived backend/tests/packaging
remain excluded.
