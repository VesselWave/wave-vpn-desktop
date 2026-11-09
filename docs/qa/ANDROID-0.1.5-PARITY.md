# Android 0.1.4–0.1.5 parity audit for Linux

> **In short:** Every Android change in `v0.1.3..v0.1.5` was checked against the Linux desktop at `a143a60` plus this pass. Linux already handled the three protocol items (cached-key recovery, timeout settling, optional bootstrap). It had real defects in the user-facing ones: a retained Network Lock showed as "Disconnected", helper refusals were generic, the tray could not cancel a pending reconnect, user Disconnect penalized the server, and light-theme state text failed contrast. All are fixed with offline contracts. Adjacent manifest defects found during the audit (D-10, D-11) are fixed too.

Date: 2026-11-08. Android sources: `android/` at `v0.1.5` (`5d4a753`), release notes `docs/RELEASE-v0.1.4.md` and `RELEASE-v0.1.5.md`, staff audit `docs/FORUM-AUDIT-2026-11-08.md` in the workspace, [staff comment 268266](https://airvpn.org/forums/topic/82017-wave-vpn-open-source-wireguard-client-for-android-and-linux-unofficial-v011/?do=findComment&comment=268266). Line numbers refer to this repository after the fix commit.

Contracts run with `python3 scripts/check-forum.py` (report in `target/forum/contracts.json`). D-numbers point to [DESKTOP-BUG-INTAKE.md](DESKTOP-BUG-INTAKE.md).

## Verdicts

| # | Android change | Linux verdict | Contract |
| --- | --- | --- | --- |
| P-01 | Failed-handshake recovery: refresh is best effort, deadline/cooldown never gate a cached connect | **Already correct**; adjacent defect D-05 fixed | `forum_p01_…`, `forum_d05_…` |
| P-02 | Manifest timeout settles; endpoint backoff without penalizing user cancellation | **Already correct** for requests; not applicable (no per-endpoint memory). Sign-in has no overall deadline: D-09 | `forum_p02_…` |
| P-03 | Optional bootstrap/routing validated separately from servers | **Already correct**; adjacent defects D-10, D-11 fixed | `forum_p02_…`, `forum_d10_…`, `forum_d11_…` |
| P-04 | Explain refused/revoked VPN permission | **Defect, fixed** (D-06) | `forum_d06_…` (4) |
| P-05 | Visible retained traffic blocking with Stop | **Defect, fixed** (D-03) | `forum_d03_…` (2) |
| P-06 | Refuse an empty selection instead of widening | **Already correct** (never widens); refusal message improved (D-07) | `forum_d07_…` |
| P-07 | Cancel reachable and accessible | **Defect in tray, fixed** (D-04). Screen readers: platform limit, D-15 | `forum_d04_…` |
| P-08 | Secondary text contrast | **Defect, fixed** (light theme) | `forum_p08_…` |
| P-09 | Hide device setting for single-device accounts | **Already correct** | `forum_p09_…` |
| — | Instant tab switch, APK size, Quick Settings tile | Not applicable (no tab animation; tray Connect is disabled when signed out, `src/tray.rs:83-94`) | — |

## Details

### P-01 Failed-handshake recovery

Android gated the next Connect on a 3-second account refresh and rethrew its timeout as cancellation. Linux has no such gate:

- `Wave::profile` (`src/gui.rs:1227`) builds the tunnel from the cached account and manifest; Connect never waits for a request.
- Refreshes are skipped while connecting (`src/gui.rs:911`); a failed refresh keeps the cached data and backs off with jitter (`src/gui.rs:614`).
- A handshake failure ends the worker as a retryable connection failure (`src/platform/linux.rs:943`, `:960`). The GUI retries with 3 s → 120 s jittered backoff (`src/lifecycle.rs:4-10`, `src/gui.rs:798`) and penalizes the failed server so Best and country targets fail over.

**Adjacent defect (D-05, fixed):** the penalty was applied on every worker end, including a clean user Disconnect or Cancel, so pressing Disconnect then Connect moved a Best or country target to a different server for 30 s. Now only a failure or an explicit switch penalizes (`src/gui.rs:784`).

### P-02 Timeout settling and endpoint backoff

`Client::request` (`src/client.rs:201`) tries each distinct bootstrap address once per request, pausing 250 ms → 2 s between attempts (`src/bootstrap.rs:411`), and stops between attempts on cancellation (`src/client.rs:217`, `:234`, `:265`). Each attempt is bounded by the HTTP timeout (10 s). Sign-in always settles: `Event::Login` clears `busy` on success and failure (`src/gui.rs:563`). There is no cross-request endpoint memory, so there is nothing for a user cancellation to penalize. Staff accepted this design in 0.1.3.

Open: a sign-in against blackholed endpoints can show "Signing in…" for about two minutes with no Cancel (D-09).

### P-03 Optional bootstrap validated separately

`parse_manifest` keeps the bootstrap only when it validates (`src/documents.rs:189`), and an invalid one never replaces the stored latest set, which falls back to the embedded definition (`src/bootstrap.rs:379`). Covered by `tests/bootstrap.rs` and `forum_p02_invalid_optional_bootstrap_keeps_valid_servers`.

**Adjacent defects (fixed):**

- **D-10:** one unparsable address in any server's `ips_entry`/`ips_exit` rejected the whole manifest, so sign-in and every refresh failed. Addresses are now skipped individually, as Eddie's `IpAddresses.Add` does (`src/documents.rs:119`). One malformed WireGuard mode is skipped as well (`:166`).
- **D-11:** a manifest with no servers or no WireGuard mode parsed successfully and replaced the cached list, leaving nothing to connect to. It is now rejected (`src/documents.rs:174`), so the refresh fails and the cache stays.

### P-04 Refused or revoked permission (helper analog)

Linux has no VPN consent. The analogs are a missing helper, a declined polkit prompt, a missing pkexec, a helper authorized for another user, and an outdated helper. Before, the GUI showed "Protection status is unavailable" or "Connection stopped unexpectedly; inspect Logs before retrying". Now:

- With the helper not installed, Overview explains that it is set up with one administrator prompt on first Connect (`Wave::protection_note`, `src/gui.rs:1449`; `HELPER_MISSING`).
- Helper refusals seen during a connection, or in a privileged action's error, map to specific explanations with the fix (`helper_problem`, `src/gui.rs:1809`). A refusal is never retried (`src/gui.rs:780`).
- A missing `/usr/bin/pkexec` is reported as such instead of "No such file or directory" (`service::authorization_available`, `src/service.rs:35`, used at `:62`).

Proactive detection of version skew after a package upgrade is not implemented; see D-08.

### P-05 Retained traffic blocking

With Network Lock confirmed on and no tunnel, Overview said "Disconnected" with a small grey sentence, and turning the lock off required finding Settings. Now `Wave::display_status` (`src/gui.rs:1436`) reads **Blocking traffic** in the status chip, the sidebar (`src/views.rs:186`, `:255`) and the tray (`Wave::tray_snapshot`, `src/gui.rs:1462`). The warning-coloured note says apps have no internet, and an outlined **Turn off Network Lock** button sends the explicit `lock-disable` action (`src/views.rs:346-370`). The state is claimed only from a confirmed status, never from the saved preference (`src/gui.rs:1431`).

### P-06 Empty selection

`Wave::selected` (`src/gui.rs:485`) returns nothing when the target matches no available server, and Connect refuses. It never widens to another server or country. The hidden country allow/deny lists that could empty the candidate set are reset to defaults by the GUI (`src/options.rs:427`). The refusal was the generic "No available server matches this target"; it now names the target and suggests Best server (`Wave::no_server_reason`, `src/gui.rs:1489`).

The Favorites and search views can show an empty list without explanation. The target and the Connect label are unchanged, so nothing widens. This is cosmetic and documented in D-07.

### P-07 Cancel reachability and accessibility

The power button cancels while connecting and during a reconnect wait (`Wave::power_cancels`, `src/gui.rs:1472`; `src/views.rs:244`). **Defect (D-04, fixed):** the tray offered **Connect** during the reconnect wait, which started a connection at once and skipped the backoff. It now offers **Disconnect**, which cancels the retry. A side effect: the tray's Recover entry is greyed during that wait, but Overview's Recover still works.

Iced 0.14 exposes no AT-SPI accessibility tree and no keyboard focus for buttons, so Orca cannot read or reach any control. This is a platform limit, documented as D-15. The tray menu is rendered by the desktop shell and stays accessible.

### P-08 Contrast

Linux used the dark warning, danger and success colours in the light theme: warning text was 2.0:1 on white and danger 3.0:1. Log timestamps and levels used the faint token: 2.5:1 light, 3.7:1 dark. `theme::color` (`src/theme.rs:70-95`) now uses Android's light tokens (warning `#B45309`, danger `#D92D20`, good `#16A34A`, accent text `#1F57E6`). Log time and level text use muted (`src/views.rs:905-915`), as Android's `LogsScreen` does. The contract checks that muted, accent, warning and danger text are at least 4.5:1 on every surface in both themes. Dark tokens are unchanged.

### P-09 Device setting

The device picker appears only for accounts with more than one device (`Wave::device_choices`, `src/gui.rs:1476`; `src/views.rs:313`). A device removed on the website is replaced automatically (`src/gui.rs:1322`).

## Not verified here

All checks ran on macOS (no VM, no provider). The Linux-only code paths touched (`src/service.rs:62`) were not compiled for Linux in this pass, because the `ring` cross-build needs an aarch64 C toolchain. That code is one call to a function that is compiled and tested on macOS. GUI rendering was checked only in macOS preview screenshots of the retained-lock scene, in dark and light themes. Tray, polkit and package behavior still need the VM items in the intake.
