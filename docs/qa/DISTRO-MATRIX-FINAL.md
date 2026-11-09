# Final distribution matrix (build 8735ec9)

> **In short:** status: in progress. Final round on disposable lab guests with a live AirVPN account, after all B-, R- and N-03/N-05 fixes. Ubuntu 26.04 and Fedora 45 are done. New: **F-01 (blocker on Ubuntu 26.04)**: the stock AppArmor profile for `wg` denies reading Wave's WireGuard config, so no connection is possible out of the box.

Date: 2026-11-09. Tester: final-round test agent. Line numbers refer to `8735ec9`.

## Build

| Label | Source | Used for |
| --- | --- | --- |
| **FIX** | `main` `8735ec9`, lab build, unsigned (`.deb`, `.rpm`, `.pkg.tar.zst`, version `0.1.3-1`) | everything unless stated |
| **REL** | signed published `v0.1.3` | R-01 upgrade baseline on Ubuntu 26.04 |

## Method

Linked clones of the lab templates ([LAB-SIGMA.md](LAB-SIGMA.md)), one per distribution, destroyed when the distribution was done. Every distribution used a real password login at the display manager (`labctl login-mode password` + `dm-login`), so PAM unlocked gnome-keyring or KWallet. The package was installed with the native package manager after boot finished. Sign-in was typed into the GUI form through QMP by a script reading the private credentials file; nothing was printed or captured. GUI driven by QMP clicks; shell over SSH, or the guest agent while Network Lock blocks the LAN.

A guest script did the network checks in one pass: `tcpdump` on the physical NIC (port 53/853, and any guest-originated packet other than WireGuard to the current endpoint, SSH, DHCP, mDNS/LLMNR, ARP and IPv6 multicast), `getent hosts`, `dig`, `dig @9.9.9.9`, and `curl -4`/`curl -6` to AirVPN's "what is my IP" API (reports whether the exit is an AirVPN server, and its name). The LAN is IPv4-only to the internet (ULA IPv6), so IPv6 internet exists only through the tunnel.

Evidence (outside Git): `.lab/evidence/final/` in the private lab directory. Prefixes: `u26-`/`ubu-` Ubuntu 26.04, `f45-`/`fed-` Fedora 45.

## Matrix

| | C1 install | C2 sign-in, keyring restore | C3 connect, leaks | C4 switch ×3 | C5 disconnect, lock on | C6 kill -9 worker | C7 `nmcli con up` | C8 reboot, lock on | C9 tray | C10 single instance, `--version` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Ubuntu 26.04 GNOME | PASS | PASS | **FAIL → F-01** (PASS with a local AppArmor override) | PASS* | PASS* | PASS* | PASS* | PASS* | PASS* | PASS |
| Fedora 45 Workstation | PASS | PASS | PASS | PASS | PASS | PASS | PASS (N-04 seen) | PASS | N/A (no tray host; D-19 PASS) | PASS |

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

## N-02 observations

| Distribution | Install | Display manager stop after postinst |
| --- | --- | --- |
| Ubuntu 26.04 | FIX after boot finished; REL→FIX upgrade while connected | none |
| Fedora 45 | FIX after boot finished | none |

## New bugs

### F-01 Ubuntu 26.04: AppArmor's `wg` profile blocks Wave's WireGuard config, Connect always fails — blocker (Ubuntu 26.04)

- **Build:** FIX and REL, Ubuntu 26.04.1, `apparmor` package profile `/etc/apparmor.d/wg` (enforced; wireguard-tools 1.0.20250521).
- **Repro:** install, sign in, Connect.
- **Actual:** the banner says `wg: fopen: Permission denied` and the connection fails; kernel log `apparmor="DENIED" operation="open" profile="wg" name="/run/wave-vpn/wavevpn0.conf" requested_mask="r"`. The stock profile only allows files under `/etc/wireguard/`. Ubuntu 24.04 has no such profile, so earlier rounds passed.
- **Code:** `src/platform/linux.rs:483` runs `wg setconf wavevpn0 /run/wave-vpn/wavevpn0.conf` (`CONFIG` at `src/platform/linux.rs:27`).
- **Suggested fix:** pass the configuration on stdin: `run("wg", &["setconf", IFACE, "/dev/stdin"], Some(config_bytes))` (`run` already takes input, `src/platform/linux.rs:123`), and stop writing the private key to a file. Checked on the guest: under the same profile, `wg setconf <dev> /run/…` fails with `fopen: Permission denied`, while piping the same file to `wg setconf <dev> /dev/stdin` succeeds. Alternatively configure the device over netlink without `wg`.
- **Verify fix:** stock Ubuntu 26.04 without `/etc/apparmor.d/local/wg` changes connects; no `apparmor="DENIED"` for `wg`.
- **Evidence:** `ubu-c3a.png`, `u26-f01-apparmor-wg.txt`.

## Not run or limited

- Both packages carry version `0.1.3-1`, so R-01 reinstalls over the same version (same maintainer-script path as an upgrade).
- Fedora C9: no tray host by default; the tray was not tested with the AppIndicator extension enabled there.
