# Desktop release gates

The initial target is Ubuntu 24.04 with systemd, kernel WireGuard, nftables,
Secret Service and GNOME. x86_64 GNOME Wayland, ARM64 GNOME X11, and the
additional KDE/Xfce lab guests now have verified candidate command evidence
recorded in VERIFICATION.md. Other distributions/init systems/platforms remain
outside the initial promise. Normal EFI guest boot and boot-persistent lock
ordering have been confirmed on the release candidate. Safe physical suspend/resume
requires dedicated bare-metal Linux hardware and safe recovery, and remains open.

A candidate must identify the exact source tag/revision and tested binaries.
The Fedora isolated packet harness now passes direct and broker modes after
disabling `resolvectl` paging; same-daemon comparisons identified a client
SIGPIPE failure. See the dated verification record for exact debug-binary scope.
This does not qualify a final release GUI or broaden the supported platforms.

The final bounded installed-release GUI check passed account/error distinction,
Recover before sign-in and isolated-keyring save/restart restore. Completed
connect/cancel/switch/retry/stop, tray, sign-out and PAM login remain unqualified;
the attempted connection did not establish a final Connected result. See the
dated verification record. No production source correction followed this check.

Before publishing, record results in VERIFICATION.md for:

- Actual GUI sign-in/restore/sign-out, wrong account vs unreachable bootstrap,
  recovery before sign-in, device selection, connect/cancel/switch, retry and stop.
- DNS and IPv4/IPv6 provider checks including wrong DNS/identity, unavailable check
  service, and partial connectivity. Network Lock must survive failed checks.
- Worker/helper crash, uplink loss/return, suspend/resume, and real reboot with
  session lock vs optional persistent lock. Verify actual firewall state and DNS
  restoration, not only cached status files.
- Install/upgrade/uninstall, conflicting firewall ownership, missing dependencies
  and state loss; both supported CPU architectures and GNOME X11/Wayland.
- Latest available official Eddie profile import with default/password/keyring
  protection. Record both the source version and available platform package;
  do not relabel an older official package as a newer tested release.
- Confirm delivery to the maintainer-designated private reporting contact,
  `vesselwave@protonmail.com`. The contact and public signing identity are now
  recorded in SECURITY.md; no production release or inbox-delivery test is claimed.

## Release identity

The OpenPGP release-signing fingerprint is:

```text
414018C0BBB8C0A70A80BF570AA32CCB2C0B3951
```

The [public key](keys/wave-release-signing.asc), identity
`Wave VPN Releases <vesselwave@protonmail.com>`, expires on 2028-10-21.
Publish this fingerprint through a separately trusted maintainer channel before
public distribution. A successful signature check identifies the signing key;
it does not establish that release readiness or platform verification is complete.

Run `bash scripts/release.sh` from a clean tagged Linux checkout for an unsigned
candidate, or `bash scripts/release.sh --sign-key FULL_FINGERPRINT` to sign using
the maintainer key. GPG obtains any required passphrase privately; no
private key is created, exported or included. This does not publish anything.
Artifacts include source, binaries/notices, checksums and source/toolchain records.
The signed mode also exports the public key and verifies the resulting signature.

Users verify downloaded files with:

```sh
bash scripts/verify-release.sh wave-signing-key.asc wave-SHA256SUMS \
  wave-SHA256SUMS.asc TRUSTED_FULL_FINGERPRINT
```

Get the fingerprint through a separately trusted maintainer channel. The script
uses an isolated keyring, checks that fingerprint, verifies the checksum signature
and hashes every listed archive. A key downloaded beside an archive does not by
itself establish trust. Reproducible compilation remains a separate claim.
