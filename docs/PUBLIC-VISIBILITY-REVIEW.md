# Public visibility preparation — 2026-11-02

Maintainer choice: prepare this existing repository for public visibility.
Visibility remains PRIVATE. This document is preparation, not authorization to
publish account data or a claim that every possible secret has been excluded.

## Completed checks

- Inspected all 11 locally reachable revisions (163 unique blobs).
  No private-key PEM blocks, recognizable GitHub tokens, AWS access-key IDs or
  Google API-key patterns were found. No tracked archived-reference/build paths,
  keystores, account.json, local.properties or raw packet-capture files matched
  the targeted filename scan. Generic passwords, arbitrary API tokens and deleted
  remote-only objects are outside that narrow scan; it is not a full secret audit.
- Checked current workflows: actions pinned by commit, contents permission read-only,
  no pull_request_target job or repository signing secret use. Linux release
  workflow uploads unsigned candidates for local signing.
- Added ignore rules for private capture/key inputs and archived reference folders.
  Ignore rules do not remove already committed data or protect Actions logs/assets.
- Corrected stale release/support statements and linked current qualification gates.
- Local release-verifier tests pass with tampered files and wrong fingerprints
  rejected. Documentation links and git diff whitespace checks pass.

## GitHub surface review follow-up

Reviewed the single merged PR's title/body: release scope, warnings and test
summaries; no account values or private profiles were present. There are no
issue/review comments or Actions caches. Targeted private-key/GitHub-token/AWS-key
patterns matched no content in 144 log files across 15 workflow runs; all log
archives were available. This is not a comprehensive arbitrary-secret detector.
The retained Linux candidate artifact contains unsigned release archives for
local signing; published manifests identify the source/binary archives.

Source archive filename checks found no .env, account.json, local.properties,
archived-reference folders, keystores or raw captures, and confirmed LICENSE
and provenance records are included. Repository history still contains lab LAN
addresses, usernames, device identifiers and local paths. These are recorded
operational metadata, not a promise that all personal metadata has been removed.

## Still required before visibility change

- [x] Review descriptions, PR text, available logs, artifact inventory, release
  source archives and local history with the scope above.
- [ ] Maintainer accepts exposure of existing lab metadata and reviews collaborator
  permissions before the visibility transition. Public access is not enabled yet.
- [ ] Decide whether recorded lab LAN addresses, usernames, device identifiers and
  detailed test evidence are acceptable public information. They exist in lab or
  verification records and have not been silently rewritten out of release history.
- [ ] Confirm GPL corresponding-source/notices coverage against distributed assets;
  keep old/ and .reference/ excluded. Existing provenance records remain authoritative.
- [ ] Confirm private security-inbox receipt and recoverable OpenPGP/APK signing-key
  backups with the maintainer. Never upload private keys or passwords for this check.
- [ ] Publish the trusted signing fingerprints on a maintainer-controlled channel.
- [ ] Finish exact-artifact smoke testing and review the forum draft's warnings.
- [ ] Change visibility only after this review; verify anonymous source/download
  access and support links, then separately authorize the forum post.

No app runtime or protocol code changed during this preparation. Existing
qualification failures remain open; see [the checklist](PUBLIC-RELEASE-CHECKLIST.md).
