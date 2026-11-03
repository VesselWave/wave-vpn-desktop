# physical test laptop physical baseline — 2026-11-02

physical test laptop (not lab host) is a physical x86_64 CachyOS/Arch laptop with an active
Wayland session, AC power and `/sys/power/mem_sleep` reporting `s2idle [deep]`.
The user provided temporary SSH access and confirmed availability to wake it.
The SSH host fingerprint matched the user-supplied fingerprint before connection.
No private keys, account values or raw system/network logs are included here.

## Exact published artifact: isolated install/uninstall PASS

Transferred the already signature/checksum-verified v0.1.0 source and x86_64
binary archives. On physical test laptop their SHA-256 digests matched the signed manifest:

- Source: `da24a2151d6940fbb364b9f2cb63ffe87e25c08eb37b347146fca79728f1d0fe`
- Binary archive: `0188eeebcad5ff538d45a9c262a2077a463ccaa292cfce6651b1ac580555f6ac`

Extracted both and ran the source installer with an absolute `DESTDIR` and
explicit prebuilt binary directory, without sudo. Both installed binaries
matched the archive byte-for-byte. Staged uninstall removed them successfully.
The released CLI's `--help` launched successfully on CachyOS. No GUI launch,
live helper replacement, dependency-removal case or installed upgrade was tested.
This is an additional compatibility sample, not expanded official distro support.

Released binary hashes:

- CLI: `26fe9b8098254ba0cfd5727be5a6727f69fa9ece1af04f19f4ce93f3fc18d13e`
- GUI: `edb4ec4b6ae98b23fa547fdbb600beddd64863a67bc825721a94d68a0601eac0`

## Physical suspend/resume: baseline PASS, VPN qualification OPEN

The existing helper socket was active, no Wave GUI/worker was detected and no
recovery journal or WireGuard link was present. Installed Wave binaries differ
from v0.1.0. The old CLI rejected `network-status`, so no firewall/protection
verdict is inferred from that command. No connection or Network Lock was enabled
by this test.

Set a one-shot RTC alarm approximately 35 seconds ahead, then requested
`systemctl suspend`. SSH became unreachable. Systemd recorded actual suspend at
02:13:14 UTC and return at 02:13:53 UTC on 2026-11-03 (39 seconds). SSH returned,
the boot ID was unchanged and the existing helper socket remained active.
The system's existing sleep configuration left user sessions unfrozen; this
setting was not changed. A passing cycle does not establish overnight reliability.

This closes a basic physical hardware sleep baseline only. It does **not** close
suspend with the exact release installed, authenticated tunnel traffic, enabled
Network Lock, external IPv4/IPv6 capture or post-resume provider/DNS checks.
Those remain release gates alongside the GUI and install/upgrade matrix.

## Cleanup

Disabled the temporary RTC alarm, removed the owned temporary archive/extraction
and staging directory, confirmed installed CLI/GUI hashes unchanged, no recovery
journal and no WireGuard link. No laptop network/firewall settings, installed
application files, service units or sleep policy were changed. Temporary SSH
access remains available pending further user testing; removal directions were
provided separately.

## Further qualification — 2026-11-02

The exact published x86_64 GUI rendered `unknown`, `connected` and `diagnostics`
synthetic scenes on physical test laptop's actual Wayland compositor, each producing a valid
954×1030 PNG. Each process used a new D-Bus session and temporary config/data,
preview mode avoided real account restore or connection actions, and owned
processes were terminated after capture. These are rendering passes only.

A subsequent exact-release isolated packet suite on CachyOS failed while waiting
for its first worker connection: `Device or resource busy (os error 16)`.
No workaround or source fix was applied; this is not a passing networking result.
The suite ran in new mount/network/PID namespaces with private /run and /etc
overlay, not in a separate VM. Host resolver digest remained unchanged. Full
nftables JSON digests differed before/after; dynamic counters may explain this,
but no rule-level baseline was retained, so exact firewall equality is unproved.
Afterward there was no host Wave recovery journal, worker or WireGuard interface;
only the pre-existing named namespace and unrelated firewall tables remained.
Installed binary hashes were unchanged and the helper socket remained active.
Owned temporary test directories and captures were removed. No active-tunnel
physical suspend test was attempted.

## Harness correction and passing rerun — 2026-11-02

The EBUSY above was a test-harness defect, not an application failure: the
harness bind-mounted a private file over physical test laptop's regular `/etc/resolv.conf`,
so the file DNS backend could not rename it. The harness now writes the regular
resolver inside its private `/etc` overlay (symlinked resolvers keep a private
bind-mounted target), and creates resolved's runtime directory, which a
regular-file host never had. Application source and binaries are unchanged.

Rerun of the exact published archive (SHA-256 `0188eeebcad5ff538d45a9c262a2077a463ccaa292cfce6651b1ac580555f6ac`) on
physical Delta/CachyOS, same namespace isolation: **all 19 scenarios PASS,
exit 0**. This covers dual-stack connect/traffic/disconnect, blocked bypass,
uplink loss and long pause, LAN DNS fallback, bypass routes, forwarding/NAT,
live lock toggles, handshake timeout/expiry, EOF/SIGTERM/unknown/switch
retention, concurrent workers, repeated cycles, worker crash recovery, real
isolated systemd-resolved and resolvconf modes, and persistent-policy restore.

Host checks afterward: resolver digest unchanged; nftables ruleset identical to
the baseline once packet counters and handles are excluded; no WireGuard
interface or Wave recovery journal; helper socket still present. The temporary
test directory was removed. The pause scenario simulates resume and is not a
physical suspend test. This is a namespace suite, not GUI/tray/PAM or install
matrix qualification.
