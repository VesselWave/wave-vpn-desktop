# Release gates

What has to pass before Wave VPN for Linux can be called production-qualified. Current releases ship as early releases with the open items below. Results and exact binaries go in [verification](VERIFICATION.md); every candidate must name its exact source tag and tested binaries.

**Target platform:** Ubuntu 24.04 with systemd, kernel WireGuard, nftables, Secret Service and GNOME. Other distributions, init systems and platforms are outside the initial promise.

## Passed

- [x] Command-level evidence on x86_64 GNOME Wayland, ARM64 GNOME X11, and KDE and Xfce lab guests
- [x] Normal EFI boot, and boot-persistent lock ordering on the release candidate
- [x] Isolated packet suite in direct and helper modes, including Fedora and physical CachyOS (the shipped binary passes all 19 scenarios)
- [x] Authenticated live suite from the published source: two servers, switching, crash recovery, no escaped packets in capture
- [x] Installed-release GUI: wrong-account vs unreachable sign-in, Recover before sign-in, keyring save and restore across restart
- [x] Security inbox confirmed; signing fingerprint published on keys.openpgp.org and vpn.vwave.me

## Current development-build coverage — 2026-11-09

The [Ubuntu round](qa/DISTRO-UBUNTU-2404.md),
[round-1 re-test](qa/DISTRO-UBUNTU-2404-RETEST.md),
[round-2 re-test](qa/RETEST-ROUND2.md) and
[final matrix](qa/DISTRO-MATRIX-FINAL.md) cover GUI connection flows, PAM/keyring,
tray, install/upgrade/removal, reboot, firewall coexistence and live DNS switching
on unsigned development packages. The final blocker recheck used `3f531fe`.
The second-server DNS failure did not recur in 12 Ubuntu switches; the earlier
ARM64 observation remains unexplained. These results do not qualify an unbuilt
release binary or close physical suspend, missing-dependency/architecture cases,
connection-check fault injection or latest-Eddie import gates. Keep the exact
release-candidate checks below open until that candidate is tested.

## Still open

- [ ] **GUI flows with the release binary:** sign-in, restore and sign-out; device selection; connect, cancel, switch, retry and stop; tray; desktop login (PAM). The last attempt didn't reach a final Connected state.
- [ ] **Connection checks in the GUI:** wrong DNS or identity, unavailable check service, partial connectivity, with Network Lock surviving every failure.
- [ ] **Lifecycle:** physical suspend and resume (needs dedicated bare-metal hardware with safe recovery), real reboot with session lock vs boot lock, checking actual firewall and DNS state rather than cached status.
- [ ] **Install matrix:** install, upgrade and uninstall; conflicting firewall ownership; missing dependencies; lost state; both CPU architectures; GNOME on X11 and Wayland.
- [ ] **Eddie import** from the latest official Eddie with default, password and keyring protection. Record both the source version and the platform package actually tested; never relabel an older package as newer.
- [ ] The second-server DNS failure seen once in the ARM64 live suite: reproduce or explain.

## Release identity

Releases are signed with **Wave VPN Releases &lt;vesselwave@protonmail.com&gt;**, fingerprint `4140 18C0 BBB8 C0A7 0A80 BF57 0AA3 2CCB 2C0B 3951` ([public key](keys/wave-release-signing.asc), expires 2028-10-21). A good signature identifies who signed; it doesn't mean the gates above are complete.

## Verifying a download

```sh
bash scripts/verify-release.sh wave-signing-key.asc wave-SHA256SUMS \
  wave-SHA256SUMS.asc 414018C0BBB8C0A70A80BF570AA32CCB2C0B3951
```

The script uses a throwaway keyring, checks the fingerprint, verifies the checksum signature and hashes every listed archive. Take the fingerprint from [SECURITY.md](../SECURITY.md), the keyserver or the website, not from the download. Reproducible builds are a separate, not-yet-made claim.

## Cutting a candidate (maintainers)

Write the release body using the [release notes style](RELEASE-STYLE.md).

Normally CI does this. Set `version` in `Cargo.toml`, add `docs/RELEASE-v<VERSION>.md`, commit, and push the tag `v<VERSION>`. The [release workflow](../.github/workflows/release-build.yml) runs `scripts/release.sh --sign-key` and publishes the GitHub release, using the release notes file as its text. Versions below 1.0 are marked as prereleases. The signing key and its passphrase are stored as the `RELEASE_GPG_KEY_BASE64` and `RELEASE_GPG_PASSPHRASE` secrets in the repository's `release` environment, which only `v*` tags can use.

To build one by hand, from a clean, tagged Linux checkout:

```sh
bash scripts/release.sh                                # unsigned candidate
bash scripts/release.sh --sign-key FULL_FINGERPRINT    # signed with the maintainer key
```

It builds source and binary archives with notices, checksums and source and toolchain records. Signed mode also exports the public key and verifies its own signature. GnuPG asks for any passphrase privately; no private key is created, exported or included, and nothing is published.
