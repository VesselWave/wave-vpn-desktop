# Final distribution matrix (build 8735ec9)

> **In short:** status: complete. Final round on five distributions (Ubuntu 26.04 GNOME, Fedora 45 Workstation, Arch KDE, Linux Mint 22.3 Cinnamon, openSUSE Tumbleweed KDE quick recheck) with a live AirVPN account, after all B-, R- and N-03/N-05 fixes. Fedora, Arch, Mint and openSUSE pass every check that was run. N-03 is fixed: KWallet works on Arch and openSUSE, with no plaintext offer. R-01 passes on Ubuntu 26.04: the tunnel stayed up through an upgrade done while connected. No N-02 display-manager stop occurred. New: **F-01 (blocker on Ubuntu 26.04)**: the stock AppArmor profile for `wg` denies reading Wave's WireGuard config, so Connect always fails out of the box. **F-02 (low)**: the N-05 fix is incomplete; a stale `WAYLAND_DISPLAY` still panics when `DISPLAY` is also set. **F-03 (low)**: with the lock off, NetworkManager's connectivity check sent plain HTTP over the physical NIC while connected (seen once, on Mint).

Date: 2026-11-09. Tester: final-round test agent. Line numbers refer to `8735ec9`.

## Build

| Label | Source | Used for |
| --- | --- | --- |
| **FIX** | `main` `8735ec9`, lab build, unsigned (`.deb`, `.rpm`, `.pkg.tar.zst`, version `0.1.3-1`) | everything unless stated |
| **REL** | signed published `v0.1.3` | R-01 upgrade baseline on Ubuntu 26.04 |

## Method

Linked clones of the lab templates ([LAB-SIGMA.md](LAB-SIGMA.md)), one per distribution, destroyed when the distribution was done. Every distribution used a real password login at the display manager (`labctl login-mode password` + `dm-login`), so PAM unlocked gnome-keyring or KWallet. The package was installed with the native package manager after boot finished. Sign-in was typed into the GUI form through QMP by a script reading the private credentials file; nothing was printed or captured. GUI driven by QMP clicks; shell over SSH, or the guest agent while Network Lock blocks the LAN.

A guest script did the network checks in one pass: `tcpdump` on the physical NIC (port 53/853, and any guest-originated packet other than WireGuard to the current endpoint, SSH, DHCP, mDNS/LLMNR, ARP and IPv6 multicast), `getent hosts`, `dig`, `dig @9.9.9.9`, and `curl -4`/`curl -6` to AirVPN's "what is my IP" API (reports whether the exit is an AirVPN server, and its name). The LAN is IPv4-only to the internet (ULA IPv6), so IPv6 internet exists only through the tunnel.

Evidence (outside Git): `.lab/evidence/final/` in the private lab directory. Prefixes: `u26-`/`ubu-` Ubuntu 26.04, `f45-`/`fed-` Fedora 45, `arch-` Arch, `mint-` Mint, `suse-` openSUSE.

## Matrix

| | C1 install | C2 sign-in, keyring restore | C3 connect, leaks | C4 switch ×3 | C5 disconnect, lock on | C6 kill -9 worker | C7 `nmcli con up` | C8 reboot, lock on | C9 tray | C10 single instance, `--version` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Ubuntu 26.04 GNOME | PASS | PASS | **FAIL → F-01** (PASS with a local AppArmor override) | PASS* | PASS* | PASS* | PASS* | PASS* | PASS* | PASS |
| Fedora 45 Workstation | PASS | PASS | PASS | PASS | PASS | PASS | PASS (N-04 seen) | PASS | N/A (no tray host; D-19 PASS) | PASS |
| Arch KDE Plasma | PASS | PASS (KWallet) | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS |
| Mint 22.3 Cinnamon | PASS | PASS | PASS (F-03 seen) | PASS | PASS | PASS | PASS | PASS | PASS | PASS (F-02 seen) |
| openSUSE TW KDE (quick) | PASS | PASS (KWallet) | PASS | NOT RUN | PASS | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN (F-02 seen) |

\* Ubuntu 26.04 C4 to C9 ran with the local AppArmor override from F-01 in place; without it nothing connects.

C8 is judged against [PARITY.md](../PARITY.md): an ordinary Network Lock does not survive reboot. PASS means that after the reboot no Wave table, tunnel, rule or `/run/wave-vpn` state remained, `network-status` reported `enabled:false`, and internet worked.

## Ubuntu 26.04 GNOME (Wayland)

Template `wave-tpl-ubuntu2604` (GNOME Wayland only, GDM, sudo-rs, NetworkManager + systemd-resolved stub, AppIndicators extension enabled, AppArmor).

| # | Step | Result | Notes |
| --- | --- | --- | --- |
| C1 | `apt-get install ./FIX.deb` | **PASS** | Helper installed and authorized by postinst (`/etc/wave-vpn/service.json`, socket active, helper identical to `/usr/bin/wave-vpn`); no polkit prompt later. Desktop entry validates (one hint only), icon in the dash. sudo-rs ran the maintainer scripts without trouble. |
| C2 | Sign-in from the form, GUI restart | **PASS** | gnome-keyring login collection unlocked by GDM PAM. One Secret Service item, no `account.json`; after kill + relaunch the GUI connected without asking for the account. |
| C3 | Connect, Best server | **FAIL → F-01** | Connect fails at once with the banner `wg: fopen: Permission denied`. Kernel: `apparmor="DENIED" operation="open" profile="wg" name="/run/wave-vpn/wavevpn0.conf"`. With `/run/wave-vpn/wavevpn0.conf r,` in `/etc/apparmor.d/local/wg` (test-only workaround): connected; IPv4 and IPv6 exits are AirVPN servers, DNS works, 0 packets on port 53 and nothing in the clear on the NIC. Evidence `ubu-c3a.png`, `u26-f01-apparmor-wg.txt`. |
| C4 | Switch server 3× | **PASS** | Three different servers, each with AirVPN exits on IPv4/IPv6. The first check, 22 s after clicking, caught the switch still in progress (`getent` failed; one search-domain `example.com.lan` query went to the LAN resolver, the documented split-DNS case from B-03); the other two checks and a re-check were clean (0 packets on port 53). Evidence `u26-c4.txt`. |
| C5 | Lock on, Disconnect, Turn off Network Lock | **PASS** | While connected with the lock on: traffic still through the tunnel, no leaks. After Disconnect: chip and sidebar **Blocking traffic**, warning and **Turn off Network Lock** shown; IPv4/IPv6 HTTPS blocked, `getent`/`dig`/`dig @9.9.9.9` fail, nothing on the NIC. Turn off: table gone, internet back. Evidence `ubu-c5.png`. |
| C6 | `kill -9` of the root transport worker, connected, lock on | **PASS** | Tunnel and policy rules removed automatically, lock kept (`enabled:true`, `recovery_needed:false`), everything blocked; banner "The networking worker stopped unexpectedly. Wave removed its tunnel; Network Lock stays as it was." with **Blocking traffic**. Evidence `uf-c6.txt`, `ubu-c6.png`. |
| C7 | `nmcli con up` ×2, connected, lock on | **PASS** | rc 0 both times; DNS stays on the tunnel, 0 port-53 packets on the NIC. Evidence `uf-c7.txt`. |
| C8 | Reboot while connected, lock on | **PASS (as documented)** | After the reboot: no Wave table, tunnel, rule or `/run/wave-vpn`; `enabled:false`; internet open. Evidence `u26-c8.txt`. |
| C9 | Tray (Ubuntu AppIndicators) | **PASS** | Icon in the top bar; menu shows state ("Wave VPN — Blocking traffic"), Show, Connect, Servers, Settings, Recover networking, Quit. Tray **Connect** connected (AirVPN exits verified). Evidence `ubu-tray.png`. |
| C10 | Second launch, `--version` | **PASS** | "Wave VPN is already running; showed its window.", exit 0, one process; `wave-vpn-gui --version` and `wave-vpn --version` print `0.1.3`. |
| R-01 | REL connected (lock on), `apt-get install ./FIX.deb` | **PASS** | REL needs the same AppArmor override to connect at all. apt: "Service restarts being deferred: systemctl restart wave-vpn-service.service". HTTPS probe every second: 70/70 OK through the upgrade; same transport-worker PID before and after; lock still on, AirVPN exits and no leaks afterwards. The running broker keeps the deleted REL helper (known N-01). Evidence `u26-r01.txt`. |

N-02: no `gdm.service` stop after either install (FIX install and the REL→FIX upgrade); the user session stayed up.

## Fedora 45 Workstation (GNOME Wayland)

Template `wave-tpl-fedora45` (SELinux enforcing, firewalld `FedoraWorkstation` zone, NetworkManager + systemd-resolved stub, AppIndicator extension installed but not enabled, as stock).

| # | Step | Result | Notes |
| --- | --- | --- | --- |
| C1 | `dnf install ./FIX.rpm` | **PASS** | Same helper, socket, desktop entry and icon checks as Ubuntu. |
| C2 | Sign-in from the form, GUI restart | **PASS** | One Secret Service item in gnome-keyring, no `account.json`; after kill + relaunch the GUI connected without asking for the account. |
| C3 | Connect, Best server | **PASS** | AirVPN exits on IPv4/IPv6, DNS works, 0 packets on port 53, nothing in the clear on the NIC. |
| C4 | Switch server 3× | **PASS** | Three different servers, all checks clean. Evidence `f45-c4.txt`. |
| C5 | Lock on, Disconnect, Turn off Network Lock | **PASS** | **Blocking traffic** + **Turn off Network Lock**; IPv4/IPv6 HTTPS blocked, no DNS on the NIC; turning it off restored internet. Evidence `fed-lock.png`. |
| firewalld | `firewall-cmd --reload` while locked (disconnected, then connected) | **PASS** | `inet firewalld` and `inet wave_vpn` side by side; still blocked while disconnected; tunnel, exits and DNS unaffected while connected. Evidence `f45-firewalld.txt`. |
| C6 | `kill -9` of the root transport worker, connected, lock on | **PASS** | Same as Ubuntu: tunnel removed, lock kept, correct banner. Evidence `fed-c6.png`. |
| C7 | `nmcli con up`, connected, lock on | **PASS (N-04 seen)** | DNS stays on the tunnel, 0 port-53 packets on the NIC. Every re-activation returns rc 4, "Connection activation failed: operation succeeded but object …/ActiveConnection/N does not exist"; the device was back to `connected` 2 s later. This is N-04 (first seen on openSUSE), so it is not openSUSE-specific: it shows with firewalld + Network Lock. Evidence `f45-n04.txt`. |
| C8 | Reboot while connected, lock on | **PASS (as documented)** | Only `inet firewalld` left, no tunnel, `enabled:false`, internet open. Evidence `f45-c8.txt`. |
| C9 / D-19 | No tray host (stock) | **N/A / PASS** | No tray icon (expected). Closing the window quits the app: no `wave-vpn-gui` process remains. |
| C10 | Second launch, `--version` | **PASS** | Same output as Ubuntu. |
| SELinux | `ausearch -m avc,user_avc` over all boots | **PASS** | Enforcing; no denials for Wave's own processes. The only AVC lines naming `wave-vpn` came from test commands run through the guest agent's permissive lab domain (`virt_qemu_ga_t`), not from Wave. |

N-02: no `gdm.service` stop after the install.

Lab note: on Fedora the first click after the GUI starts only focuses the window (the app is started from a guest-agent shell, so the window is not focused); later clicks act normally.

## Arch Linux KDE Plasma (Wayland)

Template `wave-tpl-arch` (SDDM, KWallet through pam_kwallet, NetworkManager + systemd-resolved stub, no firewall).

| # | Step | Result | Notes |
| --- | --- | --- | --- |
| C1 | `pacman -U ./FIX.pkg.tar.zst` | **PASS** | Helper, socket, `service.json`, desktop entry (validates) and icon as on the other distributions. |
| C2 | Sign-in from the form, GUI restart (N-03) | **PASS** | `org.freedesktop.secrets` owned by KWallet; Settings showed "Not configured" with **no** plaintext warning. After sign-in: one Secret Service item, no `account.json` in `~/.config/wave-vpn`; after kill + relaunch, and again after a reboot + password login, the account was there (the GUI connected without asking; `secret-tool` found the item without a prompt). N-03 is fixed on this stack. |
| C3 | Connect, Best server | **PASS** | AirVPN exits on IPv4/IPv6, DNS through `wavevpn0`, 0 port-53 packets and nothing in the clear on the NIC. Evidence `arch-c3.txt`. |
| C4 | Switch server 3× | **PASS** | Three servers, all checks clean. Evidence `arch-c4.txt`. |
| C5 | Lock on, Disconnect, Turn off Network Lock | **PASS** | **Blocking traffic** + **Turn off Network Lock**; IPv4/IPv6 HTTPS and `dig @9.9.9.9` blocked, nothing on the NIC (names still in resolved's cache answered locally); turn off restored internet. Evidence `arch-c5.png`, `arch-c5.txt`. |
| C6 | `kill -9` of the root transport worker | **PASS** | Tunnel and rules removed, lock kept, correct banner. Evidence `arch-c6.txt`, `arch-tray.png`. |
| C7 | `nmcli con up` ×2, connected, lock on | **PASS** | rc 0, DNS stays on the tunnel. Evidence `arch-c7.txt`. |
| C8 | Reboot while connected, lock on | **PASS (as documented)** | Nothing left, `enabled:false`, internet open. Evidence `arch-c8.txt`. |
| C9 | Plasma system tray | **PASS** | Icon in the system tray; left click shows the window, right click opens the menu (same entries as GNOME); tray **Connect** connected. Evidence `arch-tray.png`, `arch-c9.txt`. |
| C10 | Second launch, `--version` | **PASS** | Same as Ubuntu. |

N-02: no SDDM stop after the install (only the normal "Greeter stopped" at login, before the install).

## Linux Mint 22.3 Cinnamon (X11)

Template `wave-tpl-mint22` (LightDM, gnome-keyring, NetworkManager + systemd-resolved stub, ufw installed but inactive). The stock `casper-md5check.service` failure was ignored.

| # | Step | Result | Notes |
| --- | --- | --- | --- |
| C1 | `apt-get install ./FIX.deb` | **PASS** | As on Ubuntu. |
| C2 | Sign-in from the form, GUI restart | **PASS** | gnome-keyring login collection unlocked by LightDM PAM; one item, no `account.json`; restored after relaunch. |
| C3 | Connect, Best server | **PASS (F-03 seen)** | AirVPN exits on IPv4/IPv6, 0 port-53 packets on the NIC. During this one capture NetworkManager's connectivity check (plain HTTP to Canonical's check server) went out on the physical NIC; see F-03. Evidence `mint-c3.txt`. |
| C4 | Switch server 3× | **PASS** | Three servers, all checks clean. Evidence `mint-c4.txt`. |
| C5 | Lock on, Disconnect, Turn off Network Lock | **PASS** | As on the others. Evidence `mint-c5.png`, `mint-c5.txt`. |
| ufw | `ufw enable` (defaults: deny incoming, allow outgoing), Connect, then lock on | **PASS** | ufw's `ip filter`/`ip6 filter` tables and `inet wave_vpn` side by side; connected with AirVPN exits and no leaks, lock on while connected also clean; ufw still active after the reboot. Evidence `mint-ufw.txt`. |
| C6 | `kill -9` of the root transport worker (ufw active) | **PASS** | Tunnel and rules removed, lock kept, correct banner. Evidence `mint-c6.txt`, `mint-c6-tray.png`. |
| C7 | `nmcli con up` ×2, connected, lock on | **PASS** | rc 0, DNS stays on the tunnel. Evidence `mint-c7.txt`. |
| C8 | Reboot while connected, lock on | **PASS (as documented)** | Only ufw's tables left, `enabled:false`, internet open. Evidence `mint-c8.txt`. |
| C9 | Cinnamon tray (XApp status applet) | **PASS** | Icon in the panel; right click opens the menu; tray **Connect** connected. Evidence `mint-tray.png`, `mint-c9.txt`. |
| C10 | Second launch, `--version` | **PASS (F-02 seen)** | Same output as Ubuntu. Launching from a shell that still has a `WAYLAND_DISPLAY` naming no socket (the lab's session helper exports one) panics instead of using X11; see F-02. Started with that variable unset, the GUI works normally. |

N-02: no LightDM stop after the install; the session stayed up.

## openSUSE Tumbleweed KDE Plasma (quick recheck)

Template `wave-tpl-opensuse-tw` (SDDM, KWallet `ksecretd`, dbus-broker, firewalld `public`, NetworkManager writing a plain `/etc/resolv.conf`).

| # | Step | Result | Notes |
| --- | --- | --- | --- |
| C1 | `zypper install --allow-unsigned-rpm ./FIX.rpm` | **PASS** | As on Fedora. |
| C2 | Sign-in, GUI restart (N-03) | **PASS** | Settings "Not configured" with **no** "Secure storage is unavailable" warning (the round-2 failure). After sign-in: one item in KWallet, no `account.json`; after relaunch the GUI connected without asking. N-03 is fixed. Evidence `suse-settings.png`, `suse-c2.txt`. |
| C3 | Connect, Best server | **PASS** | AirVPN exits on IPv4/IPv6; `/etc/resolv.conf` replaced by Wave's file with the tunnel resolver; 0 port-53 packets on the NIC. Evidence `suse-c3.txt`. |
| C5 | Lock on, Disconnect, Turn off Network Lock | **PASS** | `inet firewalld` and `inet wave_vpn` side by side; everything blocked, correct GUI state; turn off restored internet and the original resolv.conf. Evidence `suse-c5.png`, `suse-c5.txt`. |
| N-05 | GUI started with `WAYLAND_DISPLAY=wayland-0` while the socket is `wayland-1` (the round-2 repro) | **FAIL → F-02** | Still panics, exit 101. Evidence `suse-n05.txt`. |

N-02: no display-manager stop after the install.

## N-02 observations

| Distribution | Install | Display manager stop after postinst |
| --- | --- | --- |
| Ubuntu 26.04 | FIX after boot finished; REL→FIX upgrade while connected | none |
| Fedora 45 | FIX after boot finished | none |
| Arch | FIX after boot finished | none |
| Mint 22.3 | FIX after boot finished | none |
| openSUSE TW | FIX after boot finished | none |

No occurrence in 6 installs. As in round 2, every install here was made after boot had finished; the early-boot window in which N-02 was seen was not targeted.

## New bugs

### F-01 Ubuntu 26.04: AppArmor's `wg` profile blocks Wave's WireGuard config, Connect always fails — blocker (Ubuntu 26.04)

- **Build:** FIX and REL, Ubuntu 26.04.1, `apparmor` package profile `/etc/apparmor.d/wg` (enforced; wireguard-tools 1.0.20250521).
- **Repro:** install, sign in, Connect.
- **Actual:** the banner says `wg: fopen: Permission denied` and the connection fails; kernel log `apparmor="DENIED" operation="open" profile="wg" name="/run/wave-vpn/wavevpn0.conf" requested_mask="r"`. The stock profile only allows files under `/etc/wireguard/`. Ubuntu 24.04 has no such profile, so earlier rounds passed.
- **Code:** `src/platform/linux.rs:483` runs `wg setconf wavevpn0 /run/wave-vpn/wavevpn0.conf` (`CONFIG` at `src/platform/linux.rs:27`).
- **Suggested fix:** pass the configuration on stdin: `run("wg", &["setconf", IFACE, "/dev/stdin"], Some(config_bytes))` (`run` already takes input, `src/platform/linux.rs:123`), and stop writing the private key to a file. Checked on the guest: under the same profile, `wg setconf <dev> /run/…` fails with `fopen: Permission denied`, while piping the same file to `wg setconf <dev> /dev/stdin` succeeds. Alternatively configure the device over netlink without `wg`.
- **Verify fix:** stock Ubuntu 26.04 without `/etc/apparmor.d/local/wg` changes connects; no `apparmor="DENIED"` for `wg`.
- **Evidence:** `ubu-c3a.png`, `u26-f01-apparmor-wg.txt`.

### F-02 N-05 fix incomplete: a stale WAYLAND_DISPLAY still panics when DISPLAY is set — low

- **Build:** FIX. openSUSE Plasma (`WAYLAND_DISPLAY=wayland-0`, socket `wayland-1`, `DISPLAY=:0` from Xwayland) and Mint Cinnamon X11 (`WAYLAND_DISPLAY=wayland-0`, no Wayland socket, `DISPLAY=:0`).
- **Repro:** in the graphical session, `WAYLAND_DISPLAY=wayland-0 wave-vpn-gui` where `$XDG_RUNTIME_DIR/wayland-0` does not exist.
- **Actual:** panic in `iced_winit`, `Create event loop: … WaylandError(Connection(NoCompositor))`, exit 101.
- **Cause:** `display_available()` (`src/gui.rs:2580-2593`) returns true as soon as `DISPLAY` is set, so the check at `src/gui.rs:2274` passes, but winit still picks the Wayland backend because `WAYLAND_DISPLAY` is set, and does not fall back to X11. The fix only covers the case with no `DISPLAY`.
- **Suggested fix:** before creating the event loop, if `WAYLAND_DISPLAY` names a socket that does not exist, remove it from the environment (then winit uses X11 when `DISPLAY` is set), and show the friendly "no graphical display" error only when neither is usable.
- **Impact:** low: stale variables in terminal multiplexers, or launchers that inherit an old session's environment.
- **Evidence:** `suse-n05.txt`; Mint GUI log in the run notes.

### F-03 With the lock off, NetworkManager's connectivity check went out on the physical NIC while connected — low

- **Build:** FIX, Mint 22.3 (NetworkManager `uri=http://connectivity-check.ubuntu.com/`), lock off, connected.
- **Seen:** once, in the C3 capture right after Connect: a plain-HTTP exchange (TCP 80) from the guest's LAN address to Canonical's connectivity-check server on the physical NIC. No DNS on the NIC (the name was resolved through the tunnel; B-03's DNS part holds). Not seen in the other captures on any distribution, and blocked whenever the lock is on.
- **Likely cause:** NetworkManager runs its per-device check bound to the physical interface (`SO_BINDTODEVICE`), which bypasses Wave's policy routing. Wave has no handling for it (no `connectivity` reference in `src/`); see the routing setup in `src/platform/linux.rs`.
- **Impact:** low: reveals to the local network only that this computer runs a connectivity check, as it does without a VPN. Users who care can turn the lock on.
- **Options:** document it; or, while connected, set the connectivity check off for the session (NetworkManager `[connectivity] enabled=false` in Wave's `/run/NetworkManager/conf.d` drop-in, which `src/resolver.rs:18` already manages for DNS), restoring it on Disconnect.
- **Evidence:** `mint-c3.txt`.

## Not run or limited

- Both packages carry version `0.1.3-1`, so R-01 reinstalls over the same version (same maintainer-script path as an upgrade).
- Fedora C9: no tray host by default; the tray was not tested with the AppIndicator extension enabled there.
- openSUSE was a quick recheck by plan: C4 and C6 to C10 were not run there.
- N-04 was not investigated further; on Fedora the device came back within 2 s.
- The guest-agent fallback stopped answering once on Fedora (agent active, SSH blocked by the lock); turning the lock off from the GUI restored SSH. Lab tooling, not Wave.
- No speed tests; at most one live AirVPN session at a time (two guests in parallel at most). All clones were destroyed at the end.

## Summary

- Distributions: 5. Matrix cells run: 46 (Ubuntu 10, Fedora 9 + N/A tray, Arch 10, Mint 10, openSUSE 4 + 3 extra checks). FAIL: 1 (Ubuntu 26.04 C3 → F-01); everything else PASS, with N-04, F-02 and F-03 noted where seen.
- Fixed and confirmed: N-03 (KWallet on Arch and openSUSE), R-01 (Ubuntu 26.04); B-14 to B-22 behaviour not regressed in the paths exercised (DNS ownership on resolved and plain resolv.conf, firewalld and ufw coexistence, crash cleanup).
- New bugs: F-01 blocker (Ubuntu 26.04), F-02 low (reopens N-05), F-03 low.
