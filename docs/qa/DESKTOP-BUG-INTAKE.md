# Desktop bug intake for VM testing

> **In short:** 20 items for Ubuntu 24.04 GNOME first, then other distributions. Seven are fixed with offline contracts (D-03 to D-07, D-10, D-11); testers confirm those in the GUI. The rest need a VM, the namespace packet suite or the live provider. Blockers are D-01, D-12 (non-Ubuntu), D-13 (lock removed by a firewall reload) and D-17.

Date: 2026-11-08. Code at local `main` after the parity fix commit. Line numbers refer to that commit.

## Sources read

- The full [AirVPN forum thread](https://airvpn.org/forums/topic/82017-wave-vpn-open-source-wireguard-client-for-android-and-linux-unofficial-v011/): four posts on one page (comments 267830, 268064, 268094, 268266). No user posted a Linux bug. Staff confirmed the three 0.1.3 infrastructure fixes on both platforms, reported four Android bugs, and said the team will look at the desktop client when time allows. The Android bugs' Linux analogs are in [ANDROID-0.1.5-PARITY.md](ANDROID-0.1.5-PARITY.md).
- GitHub `VesselWave/wave-vpn-desktop` and `wave-vpn-mobile`: no issues; discussions are disabled. The only PRs are the maintainer's.
- [Release gates](../RELEASE-GATES.md), "Still open", and the [ARM64 preflight](RELEASE-PUBLIC-PREFLIGHT.md).
- Pinned Eddie 2.27.2 (`67e213b`) Linux sources: `App.CLI.Linux.Elevated/src/impl.cpp` (DNS via systemd-resolved and resolv.conf, re-applied periodically) and `Lib.Platform.Linux/NetworkLockNftables.cs`.
- Code inspection of `src/platform/linux.rs`, `src/firewall.rs`, `src/service.rs`, `src/gui.rs`, `src/tray.rs`, `src/credentials.rs` and `packaging/scripts/`.

## Status key

**Layer** is what proves an item: *offline* (Rust/Python tests on any OS), *namespace* (`tests/linux_network.py` in a Linux VM, no account), *VM GUI* (a person on the GNOME desktop), *live* (real AirVPN account).

**Priority:** *blocker* must pass before production qualification; *fix-now* is a real defect to fix in the next release; *document* is expected behavior or a limit to state in the docs.

**Status:** *fixed + test* (contract in `scripts/check-forum.py`, confirm in the VM), *needs VM*, *needs live*, *doc only*.

| ID | Item | Layer | Priority | Status |
| --- | --- | --- | --- | --- |
| D-01 | Second-server DNS failure (ARM64 live run) | live | blocker | needs live |
| D-02 | systemd-resolved: LAN-domain queries leave on the physical link | namespace, live | fix-now if public names leak | needs VM |
| D-03 | Retained Network Lock looked like plain "Disconnected" | offline, VM GUI | fix-now | **fixed + test** |
| D-04 | Tray could not cancel a pending reconnect | offline, VM GUI | fix-now | **fixed + test** |
| D-05 | User Disconnect or Cancel penalized the server | offline | fix-now | **fixed + test** |
| D-06 | Helper missing, declined, other user, outdated, no pkexec | offline, VM GUI | fix-now | **fixed + test** |
| D-07 | Target with no available server; empty Favorites/search | offline, VM GUI | document | **fixed + test** (message) |
| D-08 | Package upgrade while connected leaves an old helper | VM | fix-now | needs VM |
| D-09 | Sign-in has no overall deadline and no Cancel | offline, VM | fix-now (needs a decision) | doc only |
| D-10 | One malformed server address rejected the whole manifest | offline | fix-now | **fixed + test** |
| D-11 | A manifest without servers or WireGuard modes replaced the cache | offline | fix-now | **fixed + test** |
| D-12 | Plain resolv.conf + NetworkManager rewrites: leak or Recover loop | namespace, VM | blocker off Ubuntu | needs VM |
| D-13 | Firewall coexistence: `flush ruleset`, ufw, firewalld, Docker, libvirt | VM | blocker (flush) | needs VM |
| D-14 | Other WireGuard (wg-quick) and Tailscale interfaces | VM | document | needs VM |
| D-15 | No screen-reader or keyboard access to controls (iced) | VM GUI | document | doc only |
| D-16 | IPv6: dual-stack, IPv4-only device, RA | namespace, live | document | needs VM |
| D-17 | Suspend/resume and network changes | VM, live | blocker | needs VM |
| D-18 | Keyring locked, absent, or auto-login | VM GUI | document | needs VM |
| D-19 | No StatusNotifier host: close-to-tray and an invisible app | VM GUI | fix-now if it reproduces | needs VM |
| D-20 | Second GUI instance or relaunch while hidden in the tray | VM GUI | fix-now if it reproduces | needs VM |

## Before you start (every item)

1. Use a fresh **Ubuntu 24.04.x desktop (GNOME, Wayland session)** VM with a snapshot named `clean`. Restore it between items that change system state.
2. **Network Lock drops SSH replies and all `tailscale0` traffic.** Drive lock tests from the VM console (virt-manager or Lima's VNC), not over SSH or Tailscale. Turning on **LAN access** keeps LAN SSH working, but not Tailscale (`100.64.0.0/10` is not LAN).
3. Install the candidate build: `sudo apt install ./wave-vpn_<version>_amd64.deb` (or `arm64`). Expect `libsecret-tools`, `wireguard-tools`, `nftables` and `pkexec` to be pulled in. Record `wave-vpn --version` or the About row.
4. Evidence to attach to every report: the GUI **Support report** (Logs → Support report → Save), `sudo nft list table inet wave_vpn`, `ip -4 rule; ip -6 rule`, `ip route show table 51820`, `resolvectl status`, `ls -l /etc/resolv.conf`, `journalctl -u wave-vpn-service -b --no-pager | tail -100`, and a screenshot. Never attach credentials, `/etc/wave-vpn/*` or `/run/wave-vpn/*` contents.
5. Contract check before VM work: `python3 scripts/check-forum.py` must report 20 passed, exit 0.

---

## D-01 Second-server DNS failure on ARM64

- **Source:** [RELEASE-GATES.md](../RELEASE-GATES.md) "Still open"; [RELEASE-PUBLIC-PREFLIGHT.md](RELEASE-PUBLIC-PREFLIGHT.md) (first run failed a direct `dig` on the second server; rerun passed).
- **Code area:** switch path. The old worker's `cleanup` runs `resolvectl revert` (`src/platform/linux.rs:567`); the new worker's `configure` sets link DNS (`:491-496`). The first handshake is driven by a dummy DNS packet in `monitor` (`:858-866`). Suspected: the query lands before the new peer's first handshake, or resolved still holds the reverted link state.
- **Repro (live, ARM64 and x86_64 VMs):**
  1. `python3 scripts/verify-airvpn.py --vm VM --guest-binary /abs/path/wave-vpn` with an account that selects two different servers. Repeat 20 times per architecture.
  2. On a failure, before cleanup, capture `wg show wavevpn0`, `resolvectl status wavevpn0`, `ip rule` and `dig @<tunnel DNS> example.com +tries=1 +time=2` three times, one second apart.
- **Expected:** every run passes. Otherwise classify it: no handshake yet (latest-handshake 0), wrong link DNS, or the provider.
- **Layer / priority:** live / blocker (open release gate).

## D-02 systemd-resolved split DNS

- **Source:** Eddie `impl.cpp:160-245` sets the VPN DNS on every link and `default-route no` on non-tunnel links, and re-applies periodically. Wave sets DNS only on `wavevpn0` with routing domain `~.` (`src/platform/linux.rs:491-496`).
- **Risk:** queries for a routing or search domain that the physical link owns (for example `*.lan` from DHCP) are sent to the LAN resolver outside the tunnel. With Network Lock on, port 53 is dropped first (`src/firewall.rs:86`), so they fail instead.
- **Repro (Ubuntu 24.04, NetworkManager + resolved):**
  1. `nmcli con mod "Wired connection 1" ipv4.dns-search lan && nmcli con up "Wired connection 1"`
  2. Connect with Network Lock **off**. In a second terminal: `sudo tcpdump -ni enp1s0 port 53 or port 853`
  3. `resolvectl query example.com`, `resolvectl query printer.lan`, `getent hosts example.org`
- **Expected:** no public name appears on `enp1s0`. `printer.lan` on `enp1s0` is the documented split-DNS behavior; record it. Any public name on `enp1s0` is a fix-now leak.
- **Layer / priority:** namespace (fake peer), then live / fix-now if public names leak, otherwise document.

## D-03 Retained Network Lock visibility — fixed

- **Source:** Android 0.1.5 ("Show retained traffic blocking"); staff comment 268266 item 3.
- **Was:** after a failed connection or Disconnect with the lock on, the chip said "Disconnected"; a small grey sentence was the only hint, and turning the lock off meant finding Settings.
- **Fix:** **Blocking traffic** in the chip, sidebar and tray; a warning note; a **Turn off Network Lock** button that sends `lock-disable` (`src/gui.rs:1431-1460`, `src/views.rs:346-370`).
- **Contracts:** `forum_d03_retained_network_lock_says_traffic_is_blocked`, `forum_d03_turn_off_network_lock_from_overview_is_an_explicit_disable`.
- **VM check:** turn on Network Lock (Settings → Network lock). Connect, then Disconnect. Expect **Blocking traffic** in Overview, the sidebar and the tray title, and `curl -m5 https://example.com` to fail. Press **Turn off Network Lock**: the chip says Disconnected, curl works, and `sudo nft list table inet wave_vpn` shows `policy accept` or no table. Repeat after a failure: with Wave connecting, drop the server's UDP on the host (or `sudo ip link set enp1s0 down` in the VM before pressing Connect) and wait for "First WireGuard handshake timed out"; expect the same **Blocking traffic** state.

## D-04 Tray cancel during reconnect — fixed

- **Source:** Android 0.1.5 ("Keep Cancel connecting accessible").
- **Was:** during "Reconnecting…" the tray offered **Connect**, which started at once and skipped the backoff; there was no way to cancel from the tray.
- **Fix:** the tray shows **Disconnect** while a retry is pending (`Wave::tray_snapshot`, `src/gui.rs:1462`).
- **Contract:** `forum_d04_cancel_is_reachable_while_connecting_and_waiting_to_retry`.
- **VM check:** connect, then on the host block the VM's UDP to the server, or `sudo ip link set enp1s0 down` for 15 s and back up. When the GUI shows **Reconnecting…**, open the tray menu (Ubuntu AppIndicator). Expect **Disconnect**; choosing it stops retries (chip **Disconnected** or **Blocking traffic**).

## D-05 Disconnect penalized the server — fixed

- **Source:** Android 0.1.5 ("without penalizing user cancellation"); code inspection of `Event::End`.
- **Was:** every worker end, including a clean Disconnect, penalized the active server for 30 s, so Best or a country target picked another server on the next Connect.
- **Fix:** only failures and explicit switches penalize (`src/gui.rs:784`).
- **Contracts:** `forum_d05_user_disconnect_does_not_penalize_the_server`, `forum_d05_cancelled_connect_does_not_penalize_the_server`, `forum_d05_failed_connection_still_penalizes_and_fails_over`.
- **VM check (live):** target Best. Connect, note the server, Disconnect, Connect within 30 s. Expect the same server.

## D-06 Helper missing, declined, other user or outdated — fixed

- **Source:** Android PR #3 ("Explain refused or revoked VPN permission"); code inspection of `src/service.rs`.
- **Fix:** specific messages for each case (`helper_problem`, `src/gui.rs:1809`; `HELPER_*` constants); a missing pkexec is reported as such (`src/service.rs:35`); refusals are not retried.
- **Contracts:** `forum_d06_missing_helper_is_explained_instead_of_unknown_status`, `forum_d06_helper_refusal_explains_the_failed_connection`, `forum_d06_declined_administrator_prompt_is_explained`, `forum_d06_missing_pkexec_is_explained`.
- **VM checks:**
  1. *Missing:* fresh install via `sudo apt install` from a desktop terminal (postinst authorizes `SUDO_UID`). To test the missing case, run `sudo wave-vpn service-uninstall`, reopen Wave. Overview should explain that the helper is set up on first Connect.
  2. *Declined:* press Connect and Cancel the polkit dialog. Expect the administrator explanation and no retry loop.
  3. *Other user:* create user `b`, log in as `b`, open Wave and Connect. Expect "authorized for a different desktop user".
  4. *Outdated:* covered by D-08.
  5. *No pkexec (other distributions):* on Debian 12 minimal with a GUI, without the `pkexec` package, install from the tarball. Connect should report that pkexec is not installed.

## D-07 Target with no available server — message fixed

- **Source:** Android 0.1.5 ("Refuse Only selected with no apps"). Linux analog: a target that matches nothing.
- **Fix:** the refusal names the country or server and suggests Best server (`Wave::no_server_reason`, `src/gui.rs:1489`). Nothing widens.
- **Contract:** `forum_d07_target_matching_no_server_is_refused_with_its_name`.
- **Documented:** Servers → Favorites with no favorites, or a search with no match, shows an empty list without explanation. The target and the Connect label are unchanged.
- **VM check (live):** Servers → Countries, pick a country whose servers are all closed if one exists (or edit `target` in `~/.config/wave-vpn/preferences.json` to `{"Server":"Nonexistent"}` while Wave is closed). Connect: expect the named refusal and no worker start (`pgrep -f transport-worker` empty).

## D-08 Package upgrade while connected or locked

- **Source:** `packaging/scripts/postinstall.sh`: while connected it only prints "Disconnect, then run: sudo wave-vpn service-install". GNOME Software and unattended-upgrades never show that message. The GUI has no version check against `/usr/local/libexec/wave-vpn-helper`.
- **Repro:**
  1. Install the previous release `.deb`, sign in, Connect.
  2. `sudo apt install ./wave-vpn_<new>.deb`. Record the postinst output.
  3. `cmp /usr/bin/wave-vpn /usr/local/libexec/wave-vpn-helper` (expect "differ").
  4. Disconnect, then Connect from the new GUI. Record the result. With the D-06 fix, a refusal should name the helper and the fix.
  5. Restore `clean`. Install the old release, turn Network Lock on, stay disconnected, upgrade. Expect `curl` to fail before, during (`while true; do curl -m2 -s -o /dev/null -w '%{http_code}\n' https://example.com; sleep 1; done`) and after; the GUI to show **Blocking traffic** within 15 s; the helper to be refreshed (`cmp` identical).
- **Expected fix:** refresh or flag the helper on the next idle Connect when it differs from the app binary.
- **Layer / priority:** VM / fix-now.

## D-09 Sign-in has no overall deadline and no Cancel

- **Source:** staff comment 268266 item 1 (Android Connect stuck behind a request deadline); code inspection.
- **Code area:** `Client::request` (`src/client.rs:201`) tries every distinct address, 6 embedded plus up to 16 from the latest manifest, each bounded by `http_timeout` (10 s) with 0.25–2 s pauses. Sign-in shows "Signing in…" with the button disabled (`src/views.rs`, Settings → Account) and no Cancel. Connect is unaffected once signed in, but after a restart there is no cached account, by design, so Connect needs this sign-in.
- **Repro:** sign out. On the VM, blackhole the bootstrap addresses: `sudo nft add table inet t; sudo nft add chain inet t o '{ type filter hook output priority 0; }'; sudo nft add rule inet t o tcp dport 80 drop`. Sign in and time until the error appears. Clean up with `sudo nft delete table inet t`.
- **Expected:** record the duration. More than about 60 s needs a decision: an overall deadline or a Cancel.
- **Layer / priority:** offline-measurable, VM / fix-now (needs a decision).

## D-10 Malformed server address — fixed

- **Source:** audit of `parse_manifest` against Eddie `Lib.Core/IpAddresses.cs` (`Add` skips invalid entries).
- **Was:** one bad address anywhere rejected the whole server list, so sign-in and all refreshes failed.
- **Fix:** skip per address and per malformed WireGuard mode (`src/documents.rs:119`, `:166`).
- **Contracts:** `forum_d10_one_malformed_server_address_keeps_the_other_servers`, `forum_d10_server_left_without_entries_refuses_connect_clearly`, `forum_d11_malformed_wireguard_mode_is_skipped_not_fatal`.
- **VM check:** none needed (offline).

## D-11 Empty manifest replaced the cache — fixed

- **Source:** Android `Documents.kt` requires servers and modes; Linux did not.
- **Fix:** a manifest without servers or WireGuard modes fails (`src/documents.rs:174`); the GUI keeps the cache and backs off.
- **Contracts:** `forum_d11_manifest_without_servers_is_rejected_so_the_cache_survives`, `forum_d11_manifest_without_wireguard_modes_is_rejected`.

## D-12 Plain resolv.conf with NetworkManager

- **Source:** Eddie re-applies DNS periodically (`impl.cpp:160`, "called also periodically"). Wave writes `/etc/resolv.conf` once (`src/platform/linux.rs:509-527`) and, on cleanup, refuses to restore a file changed by someone else (`:597`). Recovery then stays incomplete and the lock is retained.
- **Risk:** without systemd-resolved (Debian 12 GNOME default, some Arch setups), NetworkManager rewrites `/etc/resolv.conf` on DHCP renewal or reconnection. With the lock **off**, DNS then goes to the LAN resolver outside the tunnel: the `suppress_prefixlength 0` rule keeps the LAN subnet route. With the lock **on**, DNS fails. Every Recover reports "Resolver changed outside Wave".
- **Repro (Ubuntu stand-in):**
  1. `sudo systemctl disable --now systemd-resolved; sudo rm /etc/resolv.conf; printf '[main]\ndns=default\n' | sudo tee /etc/NetworkManager/conf.d/dns.conf; sudo systemctl restart NetworkManager`
  2. Connect, lock off. `cat /etc/resolv.conf` shows the tunnel DNS.
  3. `nmcli con up "Wired connection 1"`, then `cat /etc/resolv.conf` again. Run `sudo tcpdump -ni enp1s0 port 53` while doing `getent hosts example.com`.
  4. Disconnect, then Recover. Record both messages and `ls -l /etc/resolv.conf*`.
- **Expected:** no DNS on `enp1s0`, and Recover completes. Otherwise this is a blocker for non-resolved distributions; fix by re-asserting during the session and recovering safely.
- **Layer / priority:** namespace (an `/etc` overlay already exists in `tests/linux_network.py`), VM / blocker off Ubuntu, document for Ubuntu 24.04.

## D-13 Firewall coexistence

- **Source:** Eddie `NetworkLockNftables.cs` (own table; reapplied by Eddie); Wave's `inet wave_vpn` table (`src/firewall.rs:48`, output/input/forward at priority −10, forward `policy drop` under lock, `:142`).
- **Repros, one per snapshot:**
  1. *Flush:* lock on, disconnected. `sudo nft flush ruleset` (Debian's `nftables.service` restart does the same). Within 15 s, expect the GUI to show **Network lock off** or unknown (status reads real nft), not **Blocking traffic**. Run `curl` to see whether traffic leaks. A silent lock loss with no warning is a **blocker**; record it.
  2. *ufw:* `sudo ufw default deny outgoing; sudo ufw allow out 53; sudo ufw enable`, then Connect. Expect a clear failure, not a hang (the tunnel UDP is denied by ufw). `sudo ufw disable` afterwards.
  3. *firewalld:* `sudo apt install firewalld`, start it, Connect, then `sudo firewall-cmd --reload` while connected and locked. The Wave table must survive (`sudo nft list table inet wave_vpn`).
  4. *Docker:* `sudo apt install docker.io`, Connect. `sudo docker run --rm alpine wget -qO- https://ifconfig.co` should show the VPN exit. With lock on and disconnected, containers have no internet (forward drop); document.
  5. *libvirt:* `sudo apt install libvirt-daemon-system`; with the lock on, a nested guest on `virbr0` loses DHCP DNS unless LAN access is on (input chain). Document.
- **Layer / priority:** VM / blocker for repro 1, document for the rest.

## D-14 Other WireGuard and Tailscale interfaces

- **Source:** Wave uses fwmark and table 51820 with rules 18520/18521 (`src/platform/linux.rs:33`, `:466-490`), the same as wg-quick's defaults. `configure` refuses if table 51820 is in use (`:330`). Tailscale uses rules 5210–5270 and table 52.
- **Repros:**
  1. `sudo wg-quick up ./test.conf` (any peer, `Table=auto`), then Connect. Expect an explicit refusal "Routing table 51820 is already in use". The message names neither wg-quick nor the fix; note the wording.
  2. Tailscale: `curl -fsSL https://tailscale.com/install.sh | sh; sudo tailscale up`, Connect. Then `tailscale ping <peer>` and `curl https://ifconfig.co` (expect VPN exit). With an exit node set (`--exit-node`), record which default wins. With the lock on: tailnet traffic is dropped (document). Use the console (see Before you start).
- **Layer / priority:** VM / document (improve the wording if repro 1 is confusing).

## D-15 Screen reader and keyboard access

- **Source:** Android 0.1.5 (TalkBack and Switch Access for Cancel). Iced 0.14 has no AT-SPI tree and no keyboard focus for buttons.
- **Repro:** Settings → Accessibility → Screen Reader on (Orca). Tab through Wave. Expect nothing announced except text inputs. The tray menu (shell-rendered) is readable.
- **Layer / priority:** VM GUI / document (state in SUPPORT.md; upstream iced limitation).

## D-16 IPv6

- **Source:** `src/firewall.rs:73-82` (`in-block` drops IPv6 when the device has no IPv6 address), `src/platform/linux.rs:466-490` (`::/0` in table 51820).
- **Repros:** give the libvirt network IPv6 (`<ip family='ipv6' address='fd00:1::1' prefix='64'>` with DHCPv6 or RA). (a) Connect with a dual-stack device: `curl -6 https://ifconfig.co` shows the VPN IPv6. (b) Lock on, disconnected: `ping -6 -c2 2001:4860:4860::8888` fails while router advertisements still arrive (`ip -6 route` keeps its default). (c) A device with IPv4 only: IPv6 is blocked while connected, with no leak (`tcpdump -ni enp1s0 ip6 and not icmp6`).
- **Layer / priority:** namespace (already in the 19-scenario suite for (b)), live for (a) / document.

## D-17 Suspend, resume and network changes

- **Source:** release gate "Lifecycle"; `monitor` ends the session on a wall-clock versus monotonic gap of more than 15 s (`src/lifecycle.rs:13`, `src/platform/linux.rs:896`). There is no NetworkManager event hook.
- **Repros (lock on, connected, `tcpdump -ni enp1s0 not udp port <server port>` running throughout):**
  1. `systemctl suspend`, wake after 60 s (`virsh dompmwakeup`, or a key on the console). Expect **Reconnecting…** then **Connected**, and no non-tunnel packets.
  2. `nmcli dev disconnect enp1s0; sleep 30; nmcli dev connect enp1s0`. Expect Stalled, then handshake expired, then a reconnect, with no leak.
  3. Switch the VM NIC to another libvirt network (new subnet) while connected.
- **Expected:** the lock holds in all three, and the GUI recovers without a manual Recover.
- **Layer / priority:** VM, live / blocker (open gate; physical suspend still needs bare metal).

## D-18 Keyring locked, absent or auto-login

- **Source:** `Storage::detect` needs `/usr/bin/secret-tool` and a Secret Service on D-Bus (`src/credentials.rs:107-145`). A load failure shows "Unable to restore your saved account. Unlock your keyring and try again." with no retry button; signing in by hand still works.
- **Repros:** (a) GNOME auto-login (Settings → Users → Automatic Login), reboot, open Wave: record the prompt or message. (b) Lock the keyring (Seahorse → Login → Lock), restart Wave. (c) `sudo apt remove gnome-keyring`, log out and in, sign in: expect the plaintext warning before saving, and a `0600` file under `~/.config/wave-vpn/`.
- **Layer / priority:** VM GUI / document.

## D-19 No StatusNotifier host

- **Source:** `tray::spawn` starts with `visible = true` (`src/tray.rs:134`) until the watcher reports offline. `Message::Close` hides the window when the tray reports visible (`src/gui.rs:1201`), and Tick reopens it when the tray is not visible (`:877`). Ubuntu ships the AppIndicator extension; vanilla GNOME (Fedora, Debian) does not.
- **Repro:** `gnome-extensions disable ubuntu-appindicators@ubuntu.com`, log out and in. Open Wave, Connect, close the window.
- **Expected:** either the window comes back or the app quits after Disconnect. A hidden, unreachable process (`pgrep -a wave-vpn-gui` with no window) is a fix-now defect. Repeat with the extension enabled: close hides to the tray, and the tray's Show restores it.
- **Layer / priority:** VM GUI / fix-now if it reproduces.

## D-20 Second GUI instance

- **Source:** code inspection: no single-instance guard in `src/gui.rs`. The worker's exclusive lock refuses a second worker ("Another Wave networking operation is active").
- **Repro:** with Wave hidden in the tray (D-19 with the extension on), launch Wave from the app grid again. Then press Connect in both windows.
- **Expected:** record whether two GUIs run, which one shows the true state, and whether one sees the other's connection as **Blocking traffic** or an error. Two independent controllers are fix-now; the expected fix is to focus the existing window.
- **Layer / priority:** VM GUI / fix-now if it reproduces.

---

## Not covered or not verified here

- No VM or provider runs were made in this pass; every "needs VM" or "needs live" item is unverified.
- The Linux-only line changed in `src/service.rs:62` was not compiled for Linux here (no aarch64 C cross-toolchain for `ring`). The logic it calls is compiled and tested on macOS.
- AirVPN staff have not yet reviewed the desktop client (comment 268266). Their review may add items; give new items the next free D-number.
