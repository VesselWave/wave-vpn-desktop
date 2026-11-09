# Ubuntu 24.04 GNOME (Wayland) re-test of the U2404 fixes

> **In short:** status: in progress. Re-test of the fixes for B-01 to B-13 from [DISTRO-UBUNTU-2404.md](DISTRO-UBUNTU-2404.md) on disposable Ubuntu 24.04.5 GNOME Wayland guests with a live AirVPN account. New: R-01, an upgrade while connected now drops the connection on Ubuntu, because needrestart restarts the networking service after the B-01 fix replaces the helper.

Date: 2026-11-09. Tester: re-test agent. Line numbers refer to `8d32d4a`.

## Builds

| Label | Source | SHA-256 (.deb) | Used for |
| --- | --- | --- | --- |
| **FIX** | `main` `8d32d4a` plus lab-only docs (`b834e35`), lab build, unsigned | `72e4f5f8…ee7b7188` | everything unless stated |
| **REL** | signed published `v0.1.3` | `dacec39f…98710b` | upgrade baseline (step 1) |

Both carry version `0.1.3-1`, so the "upgrade" is `apt install` reinstalling over the same version (same prerm/postinst path as an upgrade).

## Environment

Same template as the original report: Ubuntu 24.04.5, GNOME Shell on Wayland, NetworkManager + systemd-resolved stub, AppIndicators extension, gnome-keyring, needrestart `3.6-7ubuntu4.5` (stock), 4 GiB / 2 vCPU, 1280x800. LAN: IPv4 DHCP, ULA-only IPv6. GDM password login (PAM-unlocked login keyring); the saved account was put in the keyring through `secret-tool` over stdin for steps that only need a signed-in app, so the GUI started signed in. GUI driven through QMP; shell through SSH or the guest agent while the lock blocks the LAN. Notifications were checked on screenshots with an automatic check of the banner area, validated against a screenshot that did show one, plus GNOME's notification list.

Evidence (outside Git): `.lab/evidence/u2404-retest/` in the private lab directory. File prefixes `rN-` match the step numbers below (`r1-` step 1, and so on).

## Results

| # | Fix | Result | Notes |
| --- | --- | --- | --- |
| 1 | B-01 upgrade while connected | **PASS for the helper refresh, FAIL overall → R-01** | REL connected, lock on, `apt install ./FIX.deb`: no "Disconnect, then run" line, `cmp` helper = `/usr/bin/wave-vpn` identical. But Ubuntu's needrestart then ran `systemctl restart wave-vpn-service.service` (its APT-hook "Ubuntu mode" restarts services by default); `KillMode=control-group` killed the worker, so the tunnel dropped. HTTPS went 200 → 000 (lock held, no leak). The GUI said "Connection stopped…/needs cleanup" (REL GUI) and "Networking service connection was lost" + **Cleanup needed** (FIX GUI). Reproduced 2/2 (REL→FIX and FIX→FIX). After Recover + Connect the transport-worker ran the new helper (exe not "(deleted)"). FIX over FIX while connected where needrestart did not run (the APT run was stopped before its post-invoke hook): processes kept running on "(deleted)" executables, the connection stayed up; Disconnect, then 10 s later: new broker PID, exe not "(deleted)". So the designed path works; needrestart defeats it. Evidence `r1-*`, `r1-b01-upgrade{1,2}-apt-https-journal.txt`. |
| 2 | B-02 GNOME "Connection failed" | **PASS** | FIX worker: Connect, Disconnect, Connect, 3 switches, then 4 min QMP link down with 5 failed retries. No "Connection failed" banner on any of 33 screenshots and none in GNOME's notification list (only needrestart's "Relogin or restarts required" from step 1). NetworkManager journal: `op="device-managed" interface="wavevpn0" args="false" result="success"` for every new link, no activation/assumed state changes; `nmcli dev`: `wavevpn0 wireguard unmanaged`. NM still logs a harmless `could not get wireguard properties` warning when the link is deleted. Evidence `r2-*`, `r4-down-*`, `r2-nm-journal-wavevpn0.txt`, `r2-13-gnome-notification-list.png`. |
| 3 | B-03 NM connectivity check DNS | pending | |
| 4 | B-04 failure reason | **PASS** | Link down: Logs `Error: WireGuard handshake expired` at ~126 s, then `Error: First WireGuard handshake timed out` per retry; chip **Reconnecting…**; banner shows the current reason ("First WireGuard handshake timed out" after the first retry, 14 s after expiry). No "Networking action failed" or "inspect Logs" line. Each failure is logged twice (R-02). Evidence `r4-logs-during-outage.png`, `r4-down-259s.png`. |
| 5 | B-05 stale banner | tray part **PASS**; Recover part pending | Tray → Disconnect during Reconnecting: banner cleared, status **Blocking traffic**, no reconnect after the link came back (45 s). Evidence `r5-01`, `r5-02`, `r5-03`. |
| 6 | B-06 worker kill | **PASS** | Lock on, connected, `kill -9` of the root transport-worker: `wavevpn0` gone and no 18520/18521 rules within 1 s; HTTPS 000, `wave_vpn` table kept, `enabled:true`; banner "The networking worker stopped unexpectedly. Wave removed its tunnel; Network Lock stays as it was.", chip **Blocking traffic**. Lock off: same cleanup within 1 s, HTTPS 200, default route via the physical NIC. The banner outlives Turn off Network Lock (R-03). Evidence `r6-*`. |
| 7 | B-07 Servers button | **PASS** | Connected via Best to server X with Best selected: "Connected to X" (disabled). Another server Y selected: "Switch to Y"; switch works. Current server selected: "Connected to Y" (disabled); pressing it twice left the transport-worker PID and tunnel unchanged. Evidence `r2-05`, `r2-06`, `r2-12`, `r7-01`. |
| 8 | B-09/B-13 uninstall | pending | |
| 9 | B-10 version/args | pending | |
| 10 | B-11 idle CPU | pending | |
| 11 | D-09 sign-in deadline | pending | |
| 12 | Regression pass | pending | |

## New bugs

### R-01 Upgrade while connected drops the connection on Ubuntu (needrestart restarts the networking service) — fix-now

- **Build:** REL → FIX and FIX → FIX, Ubuntu 24.04 stock needrestart.
- **Repro:** lock on, Connect, `sudo apt install ./wave-vpn_0.1.3-1_amd64.deb` (also what unattended-upgrades does).
- **Actual:** postinst now replaces `/usr/local/libexec/wave-vpn-helper` while connected (B-01 fix). The running broker and worker then map a deleted executable, so needrestart's APT hook prints `Restarting services... systemctl restart wave-vpn-service.service` and restarts the unit. With `KillMode=control-group` the worker dies with the broker: `wavevpn0`, its rules and routes go away, `recovery.json` stays, nothing reconnects. With the lock on, traffic is blocked (HTTPS 000, no leak); with the lock off the machine would silently fall back to the physical link. FIX GUI: red "Networking service connection was lost", chip **Cleanup needed** with "its tunnel may still be active", although the tunnel is already gone. The B-06 auto-cleanup cannot run because the broker that would run it was the process restarted. Before the B-01 fix (REL behaviour) the helper was not replaced, needrestart had nothing to restart, and the connection survived the upgrade.
- **Evidence:** `r1-b01-upgrade1-apt-https-journal.txt`, `r1-b01-upgrade2-apt-https-journal.txt`, `r1-05-after-upgrade.png`, `r1-11-fixgui-after-upgrade2.png`.
- **Code:** `src/service.rs:208` (install while connected replaces the helper), `src/service.rs:236` (unit with `KillMode=control-group`), `packaging/scripts/postinstall.sh:11-16`.
- **Suggested fix:** ship `/etc/needrestart/conf.d/wave-vpn.conf` with `$nrconf{override_rc}{qr(^wave-vpn-service\.service$)} = 0;` (needrestart's documented per-service opt-out) and/or run the worker outside the broker's cgroup so a broker restart cannot kill a live tunnel. The broker's own exit after replacement (`src/service.rs:463`) already covers the refresh.
- **Verify fix:** the same repro keeps HTTPS 200 through the upgrade; after Disconnect the broker restarts on the new helper.

### R-02 Each connection failure is logged twice — cosmetic

- Logs show `Error: WireGuard handshake expired` immediately followed by `WireGuard handshake expired`, and the same pair for every `First WireGuard handshake timed out`. Probably the worker's `Error:` line and the GUI's own `Event::End` reason both go to the log (`src/gui.rs`, B-04 change in `eee576f`). Evidence `r4-logs-during-outage.png`.

### R-03 Worker-lost banner stays after Turn off Network Lock — cosmetic

- After B-06's banner ("…Network Lock stays as it was."), pressing **Turn off Network Lock** turns the lock off correctly but the red banner stays, now contradicting the sidebar's "Network Lock off". The B-05 fix clears the banner on Disconnect and Recover only (`src/gui.rs`, `eee576f`). Evidence `r6-03-lock-off.png`.

## Not run or limited

- Real version bump: both packages are `0.1.3-1`.
- An interactive `sudo apt install` in a pseudo-terminal could not be completed by the lab tooling (it hung after the triggers and was stopped before needrestart ran). needrestart's Ubuntu changelog documents that its APT-hook mode restarts services by default, so the non-interactive result applies to interactive installs too; not observed directly.
