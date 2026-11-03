# Public release and forum checklist

Current assessment: 2026-11-02 (Chicago). v0.1.0 is an experimental prerelease.
This is a maintainer checklist, not AirVPN admission criteria. The forum describes
[third-party software](https://airvpn.org/forums/forum/49-third-party-software/)
as community software not developed by AirVPN. No approval or endorsement is claimed.

## Before announcing an experimental build

- [x] Publish v0.1.0 tagged source, notices, binaries and signed checksum manifest;
  hosted checks and downloaded-asset verification passed in the publication record.
- [x] Provide installation, verification, known-limit and private-reporting guidance.
- [ ] Make source and downloads accessible to forum readers. GitHub repository
  visibility was PRIVATE when checked on 2026-11-02. See [visibility preparation](PUBLIC-VISIBILITY-REVIEW.md). Review history, assets,
  licenses, workflows and contact details before changing visibility; then verify
  source/download links without repository membership.
- [x] Confirm private security inbox receipt and recoverable backups of both
  signing identities (maintainer confirmed 2026-11-02: controls and receives the
  security inbox; encrypted backups exist for the OpenPGP and APK signing keys).
- [x] Publish signing fingerprints through an independently trusted channel:
  keys.openpgp.org (email identity verified) and https://vpn.vwave.me/#verify.
- [ ] Complete the exact shipped artifact's basic install/connect/disconnect/recovery
  smoke test and record its hash, platform, failures and cleanup. Historical builds
  are not substitutes for this test.
- [ ] Review the forum introduction, support ownership and acceptable experimental
  privacy scope with the maintainer. Post only after explicit authorization;
  request staff guidance if a listing/subforum requires it.

## Evidence and acceptance

Keep packet captures and credentials private. Record artifact hash, source revision,
OS/security patch, scenario, positive controls, expected/observed result and cleanup.
Never convert a failed or unqualified run into a pass by narrowing its label later.
A signed artifact establishes identity, not production qualification.

## Current preflight evidence

[Fresh public-release preflight](RELEASE-PUBLIC-PREFLIGHT.md) records the
source-release authenticated run, including its second-server DNS-query failure.
The full run did not pass. Android artifact smoke was blocked by AVD-in-use.

## New physical evidence

[physical test laptop physical baseline](RELEASE-PHYSICAL-LAPTOP.md): exact v0.1.0 staged
install/uninstall and CLI launch pass on CachyOS; a 39-second physical sleep
cycle resumes with the existing older helper socket active. Active-tunnel and
exact-installed-release suspend qualification remain open.

## Linux qualification still open

- [ ] Exercise exact downloaded x86_64 v0.1.0 binaries on Ubuntu 24.04 GNOME:
  sign-in/restore/sign-out, connect/cancel/switch/retry/stop, tray and PAM unlock.
- [ ] Install/upgrade/uninstall and missing-dependency/conflicting-firewall cases;
  retain preferences and verify DNS/firewall recovery without flushing foreign rules.
- [ ] Repeat helper/worker crash, uplink recovery and provider negative checks
  against the shipped artifact; backend/debug evidence is historical coverage.
- [ ] Physical suspend/resume with local console and a recovery path. VM pause,
  reboot and host sleep do not close this gate.
- [ ] If broadening the binary support promise, qualify each architecture/session
  and ship corresponding signed artifacts. ARM64 lab results do not qualify the
  published x86_64 binary or imply an ARM64 release exists.

Existing lab: lab host Ubuntu VM113 and Fedora VM104, plus Mac Fedora KDE and Debian
Xfce guests; see lab/README.md in the repository. Ubuntu's documented .34 SSH
address timed out during this pass; all local Lima guests were stopped. Request
current VM access before live testing. A spare Linux laptop with local console
is needed for physical sleep testing. See [release gates](../RELEASE-GATES.md),
[verification](../VERIFICATION.md) and [support](../SUPPORT.md).
