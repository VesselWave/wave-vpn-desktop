# Focused acceptance and native Linux runtime gate

2026-11-05. Production revision: `c52434f5773ada00be80c8ac0caac57c4f794c12`,
with `467e0f7` and `1a53b9e` as ancestors. Tests/evidence only on
`t3/sigma-linux-vm-tests`; no production changes, push, PR, or delegates.

## Independent acceptance

Read AGENTS.md, CONTRIBUTING.md, STAFF-REGRESSION-RECHECK.md and the relevant
lab/infrastructure instructions. Reviewed F1–F5 and the shared latency admission
and DNS-adoption budget changes. No blocking production defect found in this
focused review. The authorized desktop UID remains trusted to relax its own
lock; public-address validation does not establish provider provenance.

Compared the regression assertion changes against `467e0f7`: original mismatch,
refresh-floor, cancellation, rotation and store assertions remain enabled.
Documentation-address fixtures changed to accepted unicast fixtures, new policy
calls supply a UID, store calls supply a revision, and helper tests now confirm
success before expecting an applied set. The old `observed_*` findings now assert
the intended invariants. No original R1–R4 finding was ignored or removed.

Before starting the VM, macOS acceptance passed 128 tests, zero failures and
five pre-existing ignores across 17 targets; clippy passed. The added controller
timing test subsequently ran natively on Linux.

## Native environment and commands

The dedicated Sigma Ubuntu GNOME x86_64 guest was started using the existing
lab access. Its current address and SSH host key were obtained through the QEMU
guest agent; an existing matching SSH identity was used. No authentication,
network configuration, other VM or host configuration was changed. Boot initially
waited on `systemd-networkd-wait-online` despite NetworkManager managing the
interface; the waiting unit was stopped for this boot only. GNOME became active.
Guest-agent access remained the recovery channel throughout testing.

Tracked source, without Git metadata, private files or build outputs, was copied
to `/tmp/wave-gate-c52434f`. A clean native target directory was built using
Rust 1.99.1, Linux 6.8.0-147-generic, two Cargo jobs and
`CARGO_PROFILE_DEV_DEBUG=0`. No C-header stubs or cross-compiled evidence.

```sh
export CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked --bins
sudo python3 tests/linux_network.py target/debug/wave-vpn
sudo env WAVE_TEST_SERVICE=1 python3 tests/linux_network.py target/debug/wave-vpn
sudo python3 tests/linux_bootstrap.py target/debug/wave-vpn
bash scripts/test-keyring.sh
cargo test --locked --bin wave-vpn-gui recheck_cached_profile_and_refresh_result -- --nocapture
```

Final native all-target tests: **131 passed, zero failed, five existing ignored**,
17 targets. Clippy, fmt and the optimized release build passed. Isolated synthetic keyring tests: two passed.
Direct namespace suite: 19 PASS checkpoints; broker variant: 21. These include
dual-stack WireGuard packets, resolver modes, DNS/bypass blocking, worker
termination and crash cleanup, reconnect, concurrent-worker rejection,
Network Lock toggling, persistent-policy restore and unrelated-rule preservation.

## New path runtime evidence

`tests/linux_bootstrap.py` creates private mount/network/PID namespaces and an
`/etc` overlay. Addresses that look public are assigned only to its veth peer;
no provider or internet traffic is generated. A root-only service configuration
selects an authorized UID. Actual TCP receipts establish:

* Authorized-UID sockets reach the supplied address/port; another UID cannot.
* Literal IPv4 and IPv6 pairs rotate with exact ports and the old pair is blocked.
* A syntactically valid nft delete/create batch ending in deletion of a missing
  table fails in the kernel and leaves the original table byte-for-byte intact.
* An operation-lock conflict rejects helper lock-update, leaves the table intact,
  and a subsequent retry applies the replacement successfully.
* Recover retains the scoped policy, an explicit empty update clears supplied
  pairs, and explicit disable restores baseline packet delivery.

GUI preview rendered in the real GNOME session and produced three captures;
it stayed running until a deliberate 20-second timeout. GNOME lacked a
StatusNotifierWatcher, so tray registration was unavailable. This is render
smoke, not an authenticated desktop workflow or tray qualification.

The real controller with offline account/manifest fixtures measured 20 iterations:
cached profile generation maximum **42.18 ms**, refresh-result adoption maximum
**10.64 ms** while native builds were running. The test enforces a one-second
ceiling for each synchronous path. Existing controller/cancellation tests verify
no probes while connecting/connected/changing lock, cancellation fencing, cached
profiles after refresh failure, device reselection and update retry/backoff.
These do not measure authenticated GUI click latency or blocking HTTP cancellation.

## Bounded authenticated runtime

An existing mode-0600 private account file was validated without printing it.
Credential JSON went only to the host test's stdin; two generated profiles went
only through SSH stdin to the guest live suite. No credentials or profiles were
copied into source, arguments or committed evidence. Profiles were deleted with
their temporary directory. No fleet probes, throughput test or account changes.

Authentication passed. The complete bounded `tests/live_linux.py` suite passed:
bootstrap with broken DNS under lock; two selected servers; changed public exits;
IPv4/IPv6 identity and DNS challenges; blocked physical HTTP/HTTPS/TCP+UDP DNS;
server switch; Disconnect/Recover; worker SIGKILL; wrong DNS refused Connected;
check outage and partial IPv6 remained connected but unverified; packet capture
found no outgoing plaintext DNS/DoT or direct HTTPS probes; explicit disable
restored baseline networking.

First live attempt failed at the root-run bootstrap probe because inherited
SUDO_UID scoped embedded exemptions to the desktop UID. The rerun explicitly
cleared SUDO_UID/SUDO_USER/SUDO_GID using `sudo env -u ...`, exercising documented
direct-root behavior with no configured service UID. It passed. This is a harness
identity distinction, not evidence of successful root probing through a
desktop-scoped policy; the new namespace gate separately proves UID scoping.
The host authentication test ran twice; each run selected only two profiles.

## Installed upgrade/reboot and final baseline

The actual installer installed both native release binaries and authorized the
existing desktop UID. `WAVE_SERVICE_TEST_VM=1 python3 scripts/test-service.py`
passed all eight checkpoints, including foreign-UID rejection, saturation and
authorized progress, authorization revocation/restoration, malformed requests,
timeouts and root-only helper/config ownership.

The historical independent-repository revision
`e0791707b5d969b3b4d7b10e7f893a1399fdfe4e` was separately copied from tracked
source and built natively as a CLI. It reused the already native dependency
cache; no historical binary or archived parent backend was imported.
`tests/linux_installed_bootstrap.py prepare` installed a synthetic persistent
lock, enabled it and rotated its IPv4/IPv6 pairs. The historical broker rejected
both the new action and structured input sent through the new service client,
without changing the saved lock or active firewall. This exercises the GUI's
wire path, not a GUI click. The actual installer then replaced both the broker
and boot executable (hashes checked against the new native release), preserved
the persistent policy byte-for-byte and permitted lock-update and Recover.

Reproduce only in a dedicated disposable VM, with out-of-band recovery retained:

```sh
sudo bash scripts/install.sh install
WAVE_SERVICE_TEST_VM=1 python3 scripts/test-service.py
WAVE_SERVICE_TEST_VM=1 python3 tests/linux_installed_bootstrap.py prepare /absolute/native/old-wave-vpn "$HOME/.cache/wave-installed-state.json"
# Reboot only the dedicated guest, then reconnect as its authorized desktop user.
WAVE_SERVICE_TEST_VM=1 python3 tests/linux_installed_bootstrap.py finish "$HOME/.cache/wave-installed-state.json"
sudo bash scripts/install.sh uninstall
```

Persistent installation/removal requires root; the broker deliberately rejects
those administrative actions. The first harness attempt used the broker and
was corrected to use root for them. The historical executable also had to be
temporarily placed in a trusted root-owned executable directory for its own
installer to accept it. The test removes that staging executable afterward.

The first actual reboot restored an active persistent-lock unit, and Recover
through the QEMU guest agent returned success. This guest clears `/tmp` at boot,
so the initial source tree and saved comparison state were lost. The source and
mode-0600 state were moved to a private directory under the user's cache, the
installed native release binaries were retained there, and the installed service
suite passed again. The strict changed-boot/exact-policy assertion is repeated
from this reboot-persistent directory. Keep both source and state outside `/tmp`
when reproducing the installed reboot gate on such a guest.

On the second reboot, the guest agent verified the boot unit was active and
both exact `meta skuid` IPv4/IPv6 rules were present **before any Recover**.
The installed finish gate then passed its changed boot-ID assertion, exact saved
policy comparison and scoped-rule checks. Recover retained protection; explicit
disable and root lock-uninstall removed it. The focused packet harness passed
again against the installed optimized release binary (four PASS checkpoints).
All 40 production source/Cargo/protocol-data checksums matched the tested source.

The application installer uninstalled successfully. Final actual status was
protection disabled, no active worker and no recovery needed. The firewall
ruleset was empty as at baseline, the tunnel interface was absent, persistent
lock and service authorization files were absent, and the resolver retained its
systemd stub symlink. Before the reboot, the resolver's original checksum had
also matched after both namespace and live tests. The dedicated guest was
powered off to restore its original stopped state. No other guest was touched.

## Limits

No broad audit, full-fleet probing, throughput, hardware suspend, power-loss
durability, authenticated GUI interaction, manual PAM/keyring unlock or Best
server qualification. DNS resolution and blocking HTTP cancellation retain their
documented timeout limits. Keyring evidence uses a private bus and synthetic
collection, not the existing desktop collection. The official Eddie package was
absent, so installed-Eddie import testing was unavailable. No fallback VM needed.
This focused runtime gate does not establish general production qualification.

Copied raw test, namespace, controller-timing and keyring logs remain local under
`/tmp/wave-gate-evidence`. Initial guest build/install/upgrade logs were cleared
with guest `/tmp`; their executed outcomes were also observed in the tool output.
Later installed checks use reboot-persistent guest storage. Private live logs
are deliberately not committed.
