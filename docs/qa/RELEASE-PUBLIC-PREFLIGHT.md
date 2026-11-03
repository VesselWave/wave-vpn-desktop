# Public prerelease preflight — 2026-11-02

> **In short:** The authenticated live suite, built from the v0.1.0 source on ARM64. The first run failed one DNS query after switching servers; the rerun passed in full. The failure is unexplained and stays tracked.

## Source-release ARM64 authenticated run: INCOMPLETE / FAILED

Built the CLI from the published v0.1.0 source archive with locked dependencies
in the disposable `wave-desktop` ARM64 Ubuntu VM. This is a native ARM64 build,
not the distributed x86_64 binary; it does not expand the binary support promise.
Existing private test credentials were read locally; generated worker requests
crossed into the VM on stdin and were deleted afterward. No credentials or
profiles are included here.

An initial run stopped at the missing protocol-test-binary prerequisite after
positive uplink controls. Built that prerequisite from the same source archive
and repeated once. The subsequent run passed:

- Authenticated bootstrap, manifest and two valid server profiles.
- Isolated Internet and HTTP/TCP+UDP DNS positive controls.
- Encrypted bootstrap with broken DNS and Network Lock before connection.
- First server provider identity for IPv4/IPv6 and system DNS receipt.
- First server changed IPv4 exit, tunneled IPv6 and AirVPN DNS, with forced
  physical HTTP/HTTPS and DNS blocked.
- First server stopped with IPv4/HTTP/TCP+UDP DNS still blocked across switching.
- Second server provider IPv4/IPv6 identity and system DNS receipt.

The run then failed a direct tunnel DNS query (`dig`) on the second server.
The full switch/crash/recovery/capture suite did not complete. No passing
leak-capture or complete recovery verdict is claimed for this run. The failing
query needs reproduction/diagnosis; it is not silently attributed to the server
or treated as harmless. Temporary namespaces, veth/NAT tables, worker profiles
and source extraction were removed by cleanup; no Wave namespace, WireGuard
link or host recovery journal remained. The previously stopped VM was shut down
after the checks. Build cache remains in its pre-existing build directory.

## Other platforms

The shipped x86_64 GUI rendered synthetic states and staged install/uninstall
passed on physical CachyOS. Its isolated networking suite first failed on a
harness defect; after the harness fix, all 19 scenarios pass. See
[physical test laptop physical evidence](RELEASE-PHYSICAL-LAPTOP.md). Android's new runtime smoke
attempt was blocked by an emulator AVD-in-use error; no physical Android device
was listed. No new Android privacy or artifact qualification is claimed.

## Announcement scope

These results support only a clearly warned experimental testing invitation.
Full production qualification remains open. Do not describe rendering, signature,
staging or first-server passes as a passing complete VPN lifecycle suite.

## Source-release ARM64 authenticated rerun: PASS (earlier failure not reproduced)

Same VM, same v0.1.0-source release build (worker SHA-256
`73f343ab66d8682d467b80e42ec9ed42e47895de03ea61403df9e462552e9074`). The DNS
assertion in `tests/live_linux.py` now classifies a failure, reporting resolver
family plus UDP and TCP retry outcomes, but it still fails the run. The account
selected two different public servers this time. Full suite PASS: both servers' provider IPv4/IPv6/DNS
challenges and tunnel checks, switch retention, Disconnect/Recover, real-tunnel
SIGKILL recovery, wrong-DNS/check-outage/partial-IPv6 refusals, zero escaped
packets in the physical capture, and explicit lock-disable restoring baseline.

The earlier second-server `dig` failure remains **unexplained and intermittent**.
A single passing rerun does not prove it was server-side or harmless. Any
recurrence will now report which resolver family failed and whether a retry
over UDP or TCP succeeded. The VM was stopped afterward.
