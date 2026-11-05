# Release notes style

The GitHub release title is `Wave VPN <version> for Linux`. The body lives in
`docs/RELEASE-v<version>.md`; CI publishes that file verbatim. Use
[0.1.2](RELEASE-v0.1.2.md) as the example for future releases.

## Structure

1. Open with one or two sentences about the main benefit. Don't repeat the
   release title or start with installation instructions.
2. State release maturity, architecture and platform requirements on one line.
   Distinguish downloadable package formats from systems actually tested.
3. Write a short “What's new” section with concrete, user-visible changes. Start
   bullets with the feature or outcome; keep implementation details in the changelog.
   Separate fixes only when there are enough to warrant another section.
4. Put changes to protection defaults, breaking changes and required migration
   steps in visible text or a GitHub alert. Never hide these in a disclosure.
5. Provide a two-column download table: distribution and linked package format.
   Use the exact published asset URLs. Show rounded sizes only after checking
   them against the release assets.
6. Put copyable installation commands and checksum/signature verification in
   separate `<details>` sections. Leave blank lines around Markdown inside them.
   Keep the signing fingerprint and an independent reference for checking it.
7. Finish with user guide, version-pinned changelog, source archive and issue links.

Keep the main page scannable with disclosures closed. Avoid badges, decorative
emoji, duplicate headings, long filenames as link labels and build-tool details.
Don't include unreleased changes or imply that early releases are production-qualified.
When editing published notes, update this repository's matching file too; don't
move a tag or replace binaries for an editorial change.

## References

Reviewed on 2026-11-05 for structure and presentation; wording is original.

- [Zed](https://github.com/zed-industries/zed/releases): a short opening explains
  the release before feature and fix sections.
- [Mullvad](https://github.com/mullvad/mullvadvpn-app/releases): explicit platform
  scope, concrete change bullets, and visible notices for consequential issues.
- [IVPN](https://github.com/ivpn/desktop-app/releases): required upgrade and
  compatibility information appears before downloads.
- [RustDesk](https://github.com/rustdesk/rustdesk/releases): compact download
  tables let readers choose a build without parsing the full asset list.
- [LocalSend](https://github.com/localsend/localsend/releases): a brief summary
  identifies the purpose of a small maintenance release.

Wave uses these patterns at its own scale: a few highlights, one Linux package
table, and optional instructions below it.
