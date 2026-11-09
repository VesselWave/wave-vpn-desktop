# Ubuntu 24.04 GNOME (Wayland) distribution test

> **In short:** status: in progress. Main-distro round on disposable Ubuntu 24.04.5 GNOME Wayland x86_64 guests with a live AirVPN account. No blocker found so far: the lock held through every crash, switch, outage, upgrade and uninstall we tried, D-01 did not reproduce in 12 GUI switches, and the parity fixes D-03 to D-06 behave as specified. Confirmed defects: D-08 (a package upgrade while connected leaves an old helper that nothing ever refreshes or flags), a GNOME "Connection failed" system notification on every Disconnect/switch/retry, one public-name DNS query (NetworkManager connectivity check) on the physical link at each tunnel start, and a set of status/copy issues.

Date: 2026-11-08/09. Tester: main-distro test agent. Intake: [DESKTOP-BUG-INTAKE.md](DESKTOP-BUG-INTAKE.md) (D-ids). Line numbers refer to `b2ca704` unless stated.

## Builds tested

| Label | Source | Package | SHA-256 (.deb) | Used for |
| --- | --- | --- | --- | --- |
| **LAB** (primary) | `main` `b2ca704` (lab build, rustc 1.99.1, unsigned) | `wave-vpn_0.1.3-1_amd64.deb` | `8c966c67…cd217ed41` | everything unless stated |
| **REL** | signed published `v0.1.3` (signature and checksums verified) | `wave-vpn_0.1.3-1_amd64.deb` | `dacec39f…98710b` | upgrade baseline (D-08), D-05 comparison |
| **E194** | `main` `e194e24` (lab build, unsigned) | `wave-vpn_0.1.3-1_amd64.deb` | — | D-09 re-check only |

Both LAB and REL carry the version string `0.1.3-1`, so "upgrade" REL → LAB is `apt install ./lab.deb` reinstalling over the same version (dpkg runs the same prerm-upgrade/postinst-configure path as a real upgrade). A true version bump was not available.

## Environment

- Ubuntu 24.04.5 LTS cloud-image template, `ubuntu-desktop`, GDM + GNOME Shell on Wayland, HWE kernel 7.0, NetworkManager with systemd-resolved (stub), Ubuntu AppIndicators extension, gnome-keyring. 4 GiB / 2 vCPU, virtio-gpu (software rendering), 1280x800.
- LAN: IPv4 DHCP with a `lan` search/routing domain from the router, IPv6 ULA only (no global IPv6 route), i.e. an "IPv4-only internet" device. IPv6 internet exists only through the tunnel.
- Two guests at most, rolled back between destructive scenarios. GUI driven through QMP (screenshots, keys, clicks), shell through SSH or the guest agent when the lock blocks SSH.
- Guest deviations from stock: passwordless sudo, autologin by default (password GDM login used where stated), test tools (tcpdump, dig).
- Live account: one existing device, at most two simultaneous sessions. No extra AirVPN devices or keys were created.

## Results

Legend: PASS, FAIL (bug filed), PARTIAL, BLOCKED, NOT RUN. Build is LAB unless stated.

### A. Install, upgrade, uninstall

| # | Scenario | Result | Notes |
| --- | --- | --- | --- |
| A1 | Fresh `sudo apt install ./wave-vpn.deb` (LAB) | PASS | Helper installed and authorized for the sudo user by postinst (`/etc/wave-vpn/service.json`, socket active, helper identical to `/usr/bin/wave-vpn`); first Connect needed no polkit prompt. Desktop entry validates, icon in the app grid, tray icon on launch. |
| A2 | Dependencies pulled by apt | PASS | REL install after `apt remove wireguard-tools libsecret-tools`: both reinstalled automatically. |
| A3 | Autostart | N/A | The package installs no autostart entry and the GUI has no option; nothing to test. |
| A4 | Upgrade REL → LAB while **connected**, lock on (D-08) | **FAIL → B-01** | Tunnel and lock unaffected (HTTPS 200 every second through the upgrade). postinst prints only `Wave VPN is connected. Disconnect, then run: sudo wave-vpn service-install`; helper stays the old binary. Disconnect/Connect from the old GUI, Quit, then Connect from the new GUI: all work silently on the stale helper; nothing ever refreshes or flags it. |
| A5 | Upgrade while **disconnected, lock on** (D-08 step 5) | PASS | Helper refreshed (`cmp` identical), service restarted, HTTPS blocked before/during/after, GUI kept **Blocking traffic**. |
| A6 | `apt remove` while locked, disconnected | PASS | "disconnected, turned off Network Lock and removed the networking helper"; internet back; helper, units and `/etc/wave-vpn` gone. Running GUI is orphaned (B-09). |
| A7 | `apt purge` while **connected** and locked | PASS | Tunnel, rules, routes, link DNS and lock removed; internet back. Only `/run/wave-vpn/{bootstrap.json,worker.lock}` (tmpfs) and `~/.config/wave-vpn` remain (documented). |
| A8 | Reinstall after purge | PASS | Helper re-authorized for the sudo user. Saved preferences re-enabled the lock on the next Connect (B-13). |
| A9 | `wave-vpn --version` | FAIL → B-10 | No version flag or subcommand; the intake asks testers to record it. |

### B. GUI flows

| # | Scenario | Result | Notes |
| --- | --- | --- | --- |
| B1 | First run | PASS | Overview with Best server; Connect while signed out jumps to Settings with "Sign in to AirVPN to connect". |
| B2 | Wrong password | PASS | "AirVPN rejected the account request…", username kept, password cleared. |
| B3 | Network unreachable (D-09) on LAB | PASS (as designed at b2ca704) → B-08 | All egress except LAN blackholed: "Cannot reach or validate AirVPN sign-in…" after 55–68 s, no Cancel, password cleared, copy recommends Recover. Re-check on E194: see D-09 row. |
| B4 | Sign-in with autologin, no login keyring (D-18a) | PASS | GNOME asks "Choose password for new keyring". Cancel → "Signed in, but your account could not be saved. Unlock your keyring and sign in again."; nothing written in plain text; session stays signed in until restart. Creating the keyring saves one Secret Service item. |
| B5 | Sign-in with password GDM login (PAM) | PASS | Login keyring unlocked by PAM; account saved silently, no plaintext file. |
| B6 | Keyring restore across GUI restart | PASS | Signed in automatically after Quit/relaunch. |
| B7 | Sign-in while Network Lock blocks traffic | PASS | Works through the pinned sign-in exceptions only. |
| B8 | Connect (Best), helper preinstalled | PASS | Connected in ~15 s, IPv4/IPv6/DNS verified. |
| B9 | Helper missing (D-06.1) | PASS | Overview: "The networking helper isn't installed yet… one administrator prompt the first time you connect or change Network Lock." |
| B10 | Polkit declined (D-06.2) | PASS | Clear administrator explanation, no retry loop (no pkexec/worker afterwards). Prompt text is pkexec's generic one (B-12). |
| B11 | Polkit approved | PASS | Helper installed, connected. |
| B12 | D-05 Disconnect → Connect within 30 s on Best | PASS (LAB) | Same server. REL (old GUI) moved to another server, as expected before the fix. |
| B13 | D-03 Blocking traffic + Turn off Network Lock | PASS | Chip, sidebar and tray say **Blocking traffic**, warning note and button shown; button → Disconnected, HTTPS works, table gone, status `enabled:false`. |
| B14 | a143a60: no stale yellow warning after disconnect/unlock | PASS | |
| B15 | Switch server while connected (12×) | PASS | See D-01. Button label wrong while connected (B-07). |
| B16 | D-04 tray Disconnect during "Reconnecting…" | PASS | Tray shows **Disconnect**; choosing it stops retries; no reconnect after the network returns. Stale red banner remains (B-05). |
| B17 | Quit from tray while connected, lock on | PASS | Tunnel removed, lock kept. |
| B18 | Recover after a worker crash | PASS | Tunnel/rules/DNS removed, lock kept, IPv4/IPv6/external DNS blocked. Stale error banner stays (B-05). |

### C. Leak and protection checks (tcpdump on the physical NIC)

| # | Scenario | Result | Notes |
| --- | --- | --- | --- |
| C1 | Connected, lock off: public names, `printer.lan`, `.local`, single-label (D-02) | PARTIAL → B-03 | User queries for public names went through the tunnel. On the NIC: `printer.lan` A/AAAA to the LAN resolver (documented split DNS), `SOA local.` (nss-mdns probe), reverse PTR for the guest's own LAN addresses, and one **`AAAA connectivity-check.ubuntu.com`** at the instant the tunnel came up (NetworkManager's per-device connectivity check). No HTTP followed. |
| C2 | Connected 18 min idle, capture | PASS | No guest DNS on the NIC during idle; only WireGuard UDP plus LAN broadcast noise. |
| C3 | Lock on: lock toggled during live tunnel, GUI kill -9, worker kill -9, Recover, sign-in | PASS | Guest-originated packets on the NIC during the whole locked period: WireGuard UDP to the servers and TCP 80 to one pinned sign-in address. No DNS, no IPv6, nothing else. |
| C4 | `kill -9` GUI while connected and locked | PASS | Worker ends, tunnel and link DNS removed, lock holds, relaunch shows **Blocking traffic**. |
| C5 | `kill -9` root transport-worker while connected and locked | PARTIAL → B-06 | Lock holds and nothing leaks, but the kernel tunnel, routes and link DNS stay up and keep carrying traffic (HTTPS 200 through the tunnel) while the GUI says "Blocking traffic: apps have no internet" and asks for Recover. |
| C6 | Explicit lock off restores internet | PASS | B13. |
| C7 | D-01: ≥10 server switches with DNS checks, lock on | PASS (12/12) | Every switch: new endpoint, handshake, resolver query (random name → NXDOMAIN from the tunnel resolver), `getent`, direct `dig @tunnel-DNS`, HTTPS 200. Only WireGuard UDP on the NIC. D-01 not reproduced on x86_64. |
| C8 | D-16 IPv6, IPv4-only device | PASS | IPv6 internet through the tunnel verified; with the lock on and no tunnel, `ping -6` to a public address fails. Dual-stack LAN not available. |

### E. Lifecycle (lock on, connected, capture running)

| # | Scenario | Result | Notes |
| --- | --- | --- | --- |
| E1 | Link down 30 s (QMP `set_link`) | PASS | Tunnel survives; GUI stays "Connected" (no stall shown that early). |
| E2 | Link down 6 min | PASS | "Stalled" after ~130 s, handshake expiry ~180 s, then "Reconnecting…" with backoff. No packets left the NIC. Each retry raises a GNOME "Connection failed" notification (B-02) and the banner is generic (B-04). |
| E3 | Link down 200 s, then up | PASS | Automatic reconnect 23 s after the link returned; DNS and HTTPS OK. |
| E4 | `nmcli con down` 20 s / `up` | PASS | Tunnel survived; DNS on the tunnel link intact. |
| E5 | Connection re-activation (DHCP renew stand-in) | PASS | |
| E6 | netem 30 % loss + 500 ms delay, 40 s | PASS | HTTPS slow/timed out while impaired, normal after removal; no reconnect storm. |
| E7 | Tunnel UDP blocked 210 s, then unblocked | PASS | Reconnecting while blocked, handshake 25 s after unblock. |

### F. Other

| # | Scenario | Result | Notes |
| --- | --- | --- | --- |
| F1 | 20 min connected idle, window visible | PARTIAL → B-11 | GUI RSS flat (187 MB), 36 fds, 23 threads; helper/worker flat. GUI CPU ≈ 1.9 s per minute (≈3 % of a core) with software rendering. No journal spam (16 lines mentioning Wave over the session), no panics. |
| F2 | `wave-vpn-gui` without a display | FAIL (minor) → B-10 | Panics in `iced_winit` ("neither WAYLAND_DISPLAY nor … DISPLAY is set") instead of a clean error; also ignores unknown arguments such as `--version`. |

### Still to run in this round

D-12, NetworkManager dnsmasq, D-13 (flush, ufw, Docker, libvirt), D-14, suspend/resume, reboot with lock on, logout/login while connected, keyring across logout/reboot and sign-out, D-18 b/c, D-19, D-20, D-07, Servers search/favorites/countries, cancel during connecting, Settings/theme, close-to-tray CPU, 200 % scaling, D-09 on E194.

## Bugs

### B-01 Upgrade while connected leaves a stale helper forever (D-08) — fix-now

- **Build:** REL → LAB.
- **Repro:** install REL, sign in, turn the lock on, Connect. `sudo apt install ./wave-vpn_LAB.deb`. Disconnect and Connect in the running GUI; Quit; relaunch; Connect.
- **Expected:** the helper is refreshed at the next idle moment, or the GUI flags the mismatch and offers the one-time fix.
- **Actual:** postinst prints one terminal line (`Wave VPN is connected. Disconnect, then run: sudo wave-vpn service-install`), which GNOME Software or unattended-upgrades never show. `cmp /usr/bin/wave-vpn /usr/local/libexec/wave-vpn-helper` keeps reporting "differ" through every later Connect, Quit and relaunch; the GUI never mentions it. Here REL and LAB happen to speak the same protocol, so it works silently, which means any future helper-side fix (firewall, DNS) would silently not apply.
- **Evidence:** `/var/log/apt/term.log` excerpt above; screenshots `b06`, `b08`, `b12` (`.lab/evidence/u2404/`).
- **Code:** `packaging/scripts/postinstall.sh:8-9` (only prints); no helper/app version comparison in `src/service.rs` (`client`, `:440`) or the GUI.
- **Verify fix:** the same repro, then after Disconnect (or on the next Connect) `cmp` reports identical, or the GUI shows a specific "networking helper is out of date" message with the fix.

### B-02 GNOME "Connection failed" notification on every Disconnect, switch and retry — fix-now

- **Repro:** connect, then Disconnect (or switch server, or let a reconnect attempt fail).
- **Expected:** no system error notification for a normal Wave action.
- **Actual:** GNOME Shell shows "Connection failed — Activation of network connection failed" each time. NetworkManager assumes `wavevpn0` as an external WireGuard connection ("connection-assumed"), and when Wave deletes the link it goes `activated -> unmanaged` with "could not get wireguard properties"; GNOME reports that as a failed connection. During a long outage this repeats on every retry.
- **Evidence:** screenshots `21-connect-same-server.png`, `23-disconnect-notif-1.png`, `26-longdown-182s.png`; NetworkManager journal (`device (wavevpn0): state change: activated -> unmanaged (reason 'connection-assumed', sys-iface-state: 'external')`); `nmcli dev` lists `wavevpn0 … connected (externally)`.
- **Code:** link creation `src/platform/linux.rs:434` (`ip link add dev wavevpn0 type wireguard`) with no NetworkManager opt-out.
- **Suggested fix:** mark the interface unmanaged, e.g. ship `/etc/NetworkManager/conf.d/` `unmanaged-devices=interface-name:wavevpn0` with the helper install, or `nmcli device set wavevpn0 managed no` right after creating it.
- **Verify fix:** Disconnect, switch and a forced retry show no notification; `nmcli dev` shows `wavevpn0 … unmanaged`.
- **Related:** NetworkManager taking over the link could also reset the link's resolved settings on some versions; a candidate mechanism for D-01, not observed here.

### B-03 NetworkManager connectivity check resolves a public name on the physical link at tunnel start (D-02) — fix-now by the D-02 rule, low impact

- **Repro:** lock off, `tcpdump -ni <NIC> port 53`, Connect.
- **Actual:** one `AAAA? connectivity-check.ubuntu.com` to the LAN resolver at the moment the tunnel comes up (NetworkManager's per-device check, which resolves through the device's own link). No HTTP to the check host followed, nothing during 18 min idle. Also on the NIC (expected, record only): `printer.lan` (routing domain of the LAN link), `SOA local.`, PTR for the guest's own LAN addresses.
- **Expected (intake):** no public name on the physical link.
- **Code:** DNS only on `wavevpn0` with `~.` (`src/platform/linux.rs:491-496`); Eddie also sets `default-route no` / the VPN DNS on other links.
- **Verify fix:** the same capture across 5 connects shows no public names; or document it as a NetworkManager connectivity-check limitation in USER-GUIDE.

### B-04 Failures show "Connection stopped unexpectedly; inspect Logs before retrying" instead of the reason — minor

- **Repro:** connected, pull the network for >3 min (or block the tunnel UDP).
- **Actual:** Logs show the reason (`Error: WireGuard handshake expired`, `First WireGuard handshake timed out`) followed by `Networking action failed; inspect Logs` (circular, shown inside Logs) and `Connection stopped unexpectedly; inspect Logs before retrying`, which becomes the Overview banner and note, while the app is already retrying on its own.
- **Code:** `run_transport` turns the worker's exit status into the generic text (`src/gui.rs:1897` at b2ca704, `:1973` at 6139c3d); `src/service.rs:464`; `Event::End` displays that text (`src/gui.rs:829-838`).
- **Verify fix:** the banner names the cause ("Network lost: WireGuard handshake expired; reconnecting") and no "inspect Logs" line appears in Logs.

### B-05 Stale error banner after Recover, Disconnect or tray Disconnect — minor

- **Repro:** cause B-04 or a worker crash, then press Recover (or Disconnect from the tray).
- **Actual:** the red "Connection stopped unexpectedly…" banner stays on Overview and Servers after a successful Recover/Disconnect; there is no confirmation that Recover worked.
- **Evidence:** `17-after-recover.png`, `29-d04-linkup-45s.png`, `18-servers.png`.
- **Verify fix:** a successful Recover or user Disconnect clears the error (or replaces it with a success note).

### B-06 After the root worker is killed, an orphaned tunnel keeps carrying traffic while the GUI says "Blocking traffic" — minor

- **Repro:** lock on, connected. `sudo kill -9` the `wave-vpn-helper transport-worker` process.
- **Actual:** `wavevpn0`, its routes, rules and link DNS remain; HTTPS keeps working through the tunnel (no leak). The GUI shows **Blocking traffic** ("apps have no internet") plus "A previous Wave connection needs cleanup. Use Recover…". `network-status`: `recovery_needed:true, worker_active:false`. No automatic cleanup or retry.
- **Expected:** accurate status (e.g. "Unmanaged tunnel left over — Recover"), or automatic cleanup by the service when its worker dies.
- **Evidence:** `15-after-worker-kill.png`, `16-after-worker-kill-20s.png`.
- **Code:** `Wave::display_status` (`src/gui.rs:1436`) derives Blocking traffic from lock-enabled without looking at `recovery_needed`; the service (`src/service.rs` `handle`, `:298`) does not run cleanup when the worker it spawned exits abnormally.
- **Verify fix:** after the kill, either the tunnel is gone within 15 s or the chip and note describe the leftover tunnel.

### B-07 Servers: "Connect to X" while already connected to X reconnects needlessly — minor

- **Repro:** connected; Servers → select a server → **Connect to …** (switch works). Press the same button again.
- **Actual:** the label says "Connect to <server>" while connected (also before switching); pressing it on the current server tears the tunnel down and builds it again (new worker PIDs, a GNOME notification from B-02).
- **Code:** `src/views.rs:857`.
- **Verify fix:** label reads "Switch to …" when connected to another server and is disabled or reads "Connected" for the current one.

### B-08 Sign-in on an unreachable network (D-09) — minor, fixed on main?

- **Build:** LAB. ~55–68 s of "Signing in…", no Cancel, password field cleared afterwards; the error recommends Recover for what is a plain network outage. `e194e24`/`9532b7e` add a 45 s deadline, Cancel and keep the password; re-check pending.

### B-09 Uninstall leaves the GUI running with misleading text — minor

- **Repro:** GUI open, `sudo apt remove wave-vpn`.
- **Actual:** the GUI and its tray icon keep running from the deleted binary (dock shows a generic icon). Overview says the helper "isn't installed yet… sets it up… the first time you connect"; Connect shows "Build/install wave-vpn beside the GUI" (developer wording, `src/gui.rs:1845`).
- **Code:** `packaging/scripts/preremove.sh` does not ask running GUIs to quit.
- **Verify fix:** after removal no `wave-vpn-gui` remains, or the GUI says the app was uninstalled.

### B-10 No version flag; GUI panics without a display — doc / minor

- `wave-vpn --version` and `wave-vpn version` fail; the intake's "Before you start" step 3 asks for it (About shows 0.1.3). `wave-vpn-gui` with no display panics in `iced_winit` instead of printing a clear error and exiting non-zero.

### B-11 GUI CPU while idle and visible — minor (VM-specific magnitude)

- ≈3 % of one core continuously while connected with the window visible (software rendering on virtio-gpu, 1 s timer redraws). Memory is flat. Close-to-tray measurement pending.

### B-12 Generic polkit prompt — doc

- The one-time authorization shows pkexec's generic "Authentication is needed to run `/usr/bin/wave-vpn service-install` as the super user". A polkit action with a Wave-specific message would read better.

### B-13 Preferences outlive purge, so reinstall re-enables the lock — doc

- Uninstall says it "turned off Network Lock", but `~/.config/wave-vpn/preferences.json` keeps `netlock: true`, so the first Connect after reinstall turns the lock back on and Disconnect then blocks traffic. Consistent with "your choice carries over", but worth one sentence in USER-GUIDE "Upgrading and uninstalling".

## Not run or limited

- Real version bump upgrade: both packages are `0.1.3-1`.
- Hardware suspend/resume, GPUs, Secure Boot: not in this lab.
- Dual-stack LAN IPv6 (no global IPv6 on the lab LAN).
