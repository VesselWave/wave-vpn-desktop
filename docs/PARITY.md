# Focused desktop scope

2026-10-21: the user replaced the full Eddie/archived-app parity objective with
fewer features that can be covered properly. Android is unchanged. This is a
WireGuard Linux development build. Authenticated backend checks now pass;
remaining UI/platform limits are recorded in VERIFICATION.md.

Keep the authored Wave presentation and these workflows:

- Compact Account row with sign-in/sign-out. Automatically save and restore with
  the available keyring. If unavailable, clearly warn before storing plaintext in
  a private local file. Sign-out removes saved credentials.
- Read-only Eddie import into the form for review, including Linux keyring profiles.
  Never change Eddie's files or keyring items. Advanced Eddie settings are omitted.
- Best/country/server selection, search and favorites; connect, disconnect,
  server switching and automatic retry; accurate connection/protection status.
- Network Lock: keep protection after disconnect, cancellation, Quit, failure and
  crash. Recover removes tunnel resources while retaining the lock. A separate
  **Network Lock** toggle releases protection. It also works during a live tunnel
  and preserves the confirmed choice across future connections. Recover retains
  an enabled lock and respects an explicitly disabled lock.
- Traffic status, logs, tray, appearance and transfer units.
- One administrator authorization to install the networking helper; subsequent
  GUI networking actions run without another password prompt. This helper is
  separate from the optional boot-persistent lock.

Settings are **General** and **Network lock**, with a Network Lock toggle and five preferences:
last-server selection, appearance, transfer units, auto-reconnect and optional LAN
access. LAN and physical-interface ping bypasses default off. Existing hidden
routing, allowlist, forwarding and tuning preferences reset to safe defaults when
loaded by the GUI. Worker inputs remain validated for CLI compatibility.

Deferred: boot autoconnect; UI for boot-persistent lock installation; custom
routes/DNS/backends/forwarding; MTU/timeouts/port/scoring tuning; account-side port
forwarding, DDNS and blocklists; OpenVPN/obfuscation; other desktop VPN backends.
The optional persistent-lock CLI remains for existing installations. Reboot
persistence requires that service; an ordinary session lock does not survive reboot.

Network Lock permits only tunnel transport, pinned AirVPN bootstrap endpoints,
loopback and DHCP/IPv6 neighbor discovery by default. Optional LAN access is an
explicit exception; ordinary IPv4/IPv6 internet and external DNS remain blocked.

## Acceptance evidence

- Rust all-target tests, strict Clippy, standalone macOS UI build.
- Linux local-peer packet tests: traffic through tunnel; blocked IPv4/IPv6/DNS
  after disconnect/Recover; only explicit lock-disable restores internet; failure,
  crash, switching/repeated cycles and preservation of unrelated firewall rules.
- Ubuntu **GNOME**, working Secret Service: save/load/delete synthetic Wave account;
  import an actual installed Eddie profile, not only locally generated fixtures.
- GNOME UI: both settings tabs, sign-in/import, connection controls, protection
  status and useful layouts.
- Real AirVPN backend tests pass on Comae and Orion: dual-stack internet, AirVPN
  DNS, switching/reconnection, protected disconnect/crash/Recover and explicit
  unlock. Physical packet capture found no DNS/DoT or direct HTTPS probe leaks.
  Complete GUI retry/cancellation coverage, Wayland, reboot and suspend remain
  unverified. No credentials in chat or Git.

See [VERIFICATION.md](VERIFICATION.md) for results and exact limits. Historical
full-parity inventory is superseded, not a backlog to finish in this iteration.

## Authorized release requirements follow-up

The user authorized the must-finish items from DESKTOP-FEATURE-REVIEW.md. The
compact settings scope above remains unchanged. Provider identity/DNS checks,
actual firewall-state reconciliation, recovery/backoff, support report preview/
copy/export and signed-candidate verification are implemented. Distinguish these
changes from the environment and maintainer gates in RELEASE-GATES.md and the
latest evidence in VERIFICATION.md; this is still a development build.
