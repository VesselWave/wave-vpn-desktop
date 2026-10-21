# Source provenance

Both applications use GPL-3.0-or-later. This repository's `LICENSE` file contains GPLv3. Preserve the notices below when distributing source or binaries.

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

Server selection uses Eddie's base/load/users factors. Android optionally adds
Wave's measured TCP latency preference; it is not identical to Eddie's scoring.

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

## Linux transport reference

Linux connections invoke official Hummingbird instead of the archived helper.
Arguments and shutdown behavior were checked against
[AirVPN Suite](https://gitlab.com/AirVPN/AirVPN-Suite), commit
`35fe8af098db7e907207aa5ab6ad38219286c9cf`, `README-hummingbird.md` and
`src/hummingbird.cpp`. Hummingbird is a separate installed dependency; no Suite
source is bundled. It manages routing, DNS, WireGuard and Network Lock.

[AirVPN's third-party forum](https://airvpn.org/forums/forum/49-third-party-software/)
provides community integration context, not a protocol specification or a license
grant for projects linked there.

## Repository separation

This app has its own Git history and remote. Protocol reference data and fixtures
are included locally under `protocol-data/`; no sibling checkout is required.
The parent workspace's community checklist coordinates both applications.
