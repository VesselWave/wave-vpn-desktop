# Changelog

## Unreleased

- Fixed a brief yellow warning after disconnecting: worker and recovery warnings now require a fresh networking status instead of using flags from the previous connection.
- Android 0.1.5 parity and desktop bug intake ([parity audit](docs/qa/ANDROID-0.1.5-PARITY.md), [intake](docs/qa/DESKTOP-BUG-INTAKE.md)):
  - A retained Network Lock now reads **Blocking traffic** in Overview, the sidebar and the tray, says apps have no internet, and offers **Turn off Network Lock**
  - Disconnect and Cancel no longer penalize the server, so Best and country targets keep their choice; failures still fail over
  - The tray offers Disconnect during a pending reconnect instead of an immediate Connect
  - A missing, declined, outdated or other-user networking helper, or missing pkexec, is explained with the fix instead of a generic failure
  - A target with no available server is refused with its name and a suggestion
  - One malformed server address or WireGuard mode no longer rejects the whole server list; a list without servers or WireGuard modes is rejected so the cached one stays
  - Light theme warning, error and link text and log timestamps meet 4.5:1 contrast, matching Android
  - Sign-in gives up after 45 seconds with a clear message instead of trying every address for minutes, and offers **Cancel** while signing in, including when restoring the saved account at startup; Cancel shows no error and keeps the saved account (same behavior as Android)
  - `scripts/check-forum.py` runs the named offline contracts and writes `target/forum/contracts.json`

- Ubuntu 24.04 distribution test fixes ([report](docs/qa/DISTRO-UBUNTU-2404.md)):
  - A package upgrade while connected now updates the networking helper at once without dropping the connection, instead of leaving the old helper in place until someone ran `sudo wave-vpn service-install`; a replaced helper restarts itself once idle (B-01)
  - NetworkManager no longer adopts the tunnel link, so GNOME stops showing "Connection failed" on every Disconnect, server switch and reconnect attempt (B-02)
  - A failed connection shows its reason (for example "WireGuard handshake expired") instead of "Connection stopped unexpectedly; inspect Logs before retrying", and Logs no longer get the circular "Networking action failed; inspect Logs" line; failed Recover and Network Lock actions show the helper's reason too (B-04)
  - Disconnect (also from the tray) and a successful Recover clear the previous failure's banner (B-05)
  - When the networking worker is killed or crashes, the service now removes its leftover tunnel at once (keeping an enabled Network Lock) and says so. While a leftover connection is reported, Wave shows **Cleanup needed** and asks for Recover instead of claiming **Blocking traffic** (B-06)
  - Servers says **Switch to …** while connected and **Connected to …** (disabled) for the server already in use, instead of "Connect to …", which reconnected needlessly (B-07)
  - `wave-vpn --version` and `wave-vpn-gui --version` print the version; `wave-vpn-gui` rejects unknown arguments and, without a graphical display, exits with a clear message instead of panicking (B-10)
  - The README names the standard system prompt shown for the one-time helper authorization (B-12, documentation only)
  - When another program removes Network Lock (`nft flush ruleset`, a firewall reload), Wave no longer just shows "Network lock off". While connected, the worker restores its rules within seconds. While disconnected, Wave warns, logs the change and turns the lock back on when it's your choice. Recover keeps such a lock. Settings says when a saved "on" choice will turn the lock on at the next Connect (B-14)
  - With a plain `/etc/resolv.conf` managed by NetworkManager (the default on Debian 13 and openSUSE), DNS now stays on the tunnel for the whole session. Before, NetworkManager rewrote the file seconds after any reconnect: DNS leaked with Network Lock off while Overview said **DNS: Verified**, and failed with Network Lock on. Wave now holds NetworkManager off the file with a temporary `/run` drop-in and rewrites the tunnel DNS within a second if any program replaces the file. Recover no longer gets stuck on "Resolver changed outside Wave": it keeps a newer file another program wrote, and Connect clears a leftover backup itself (B-22)
  - Lower idle CPU use: the window no longer redraws ten times a second; background events wake it immediately and a one-second tick drives timers and the uptime display (B-11)
  - Uninstalling closes open Wave VPN windows instead of leaving one that offers first-time setup; a window whose app was removed says so (B-09). The uninstall message and user guide say that preferences, including the Network Lock choice, are kept even by `apt purge` (B-13)

## 0.1.3 · 2026-11-05 · early release

Lighter on AirVPN's infrastructure, following [AirVPN Staff feedback](https://airvpn.org/forums/topic/82017-wave-vpn-open-source-wireguard-client-for-android-and-linux-unofficial-v011/?do=findComment&comment=268064). [Release notes](docs/RELEASE-v0.1.3.md).

**Changed**

- Tunnel and DNS checks run once per connection instead of every 60 seconds; an unavailable check service leaves the connection up and unverified instead of disconnecting
- Latency probes: at most four at a time, cached for 30 minutes, never while connecting, connected or under Network Lock; unreachable servers rank last instead of counting as 0 ms
- Sign-in uses the bootstrap addresses and RSA key from the latest server list, saved locally, with the built-in set as a fallback; failover pauses grow between addresses
- Server list refresh: manifest only, every 30 minutes or AirVPN's advertised interval and never more often than every 5; manual refresh at most once a minute; failures back off up to an hour; reconnects are spread with random jitter
- Latency probes cover at most 32 servers per run, one per location in a random order, instead of the whole fleet; results and the 30-minute cooldown are kept across restarts (`~/.config/wave-vpn/latency.json`, server names and milliseconds only); unprobed servers show their location's result; a probe is abandoned within 50 ms when a connection starts
- Automatic refreshes add the account document about once a day; a manual refresh still always includes it. A device removed from the account is replaced by an existing one instead of requiring sign-out

**Fixed** (independent staff-regression review, [details](docs/STAFF-REGRESSION-REVIEW.md#resolution--fix-pass))

- An observed DNS or identity mismatch is final for that connection: a later unavailable or successful check can no longer turn it into "unverified" or "verified", the status keeps showing it, and the connection is stopped without automatic reconnect
- Jitter can no longer schedule an automatic refresh before the 5-minute floor or AirVPN's `next_update`; it only adds up to 20 %. A failed manual refresh no longer moves the automatic schedule earlier
- Network Lock follows bootstrap rotation: the latest manifest's validated sign-in IP/port pairs (and resolved addresses of sign-in hostnames) are passed to the helper and applied to the session and boot locks; unsafe addresses and DNS ports are refused
- Cancelling sign-in or refresh (sign-out) now also stops a late response from being saved, and a response from an older manifest can no longer overwrite a newer saved bootstrap

**Fixed** (independent recheck, [details](docs/STAFF-REGRESSION-RECHECK.md#resolution--repair-round-2))

- Network Lock bootstrap exceptions apply only to the authorized desktop user's apps, are shown in the Network Lock settings and diagnostics, and are documented as bounded but not provider-verified
- Client pinning and the helper's exceptions use one shared 16-address selection (every sign-in hostname keeps addresses); under Network Lock outside the tunnel an unpinned hostname is skipped instead of looked up through ordinary DNS
- A failed Network Lock update is no longer treated as applied: the previous set is kept and the update is retried while idle with backoff (30 s to 15 min), with reinstall advice for an older helper
- A late response with the same or an unknown manifest time can no longer replace a configuration saved after its request started, in any process
- Exceptions refuse every non-public address, including IPv6 site-local, discard-only, documentation, 6to4, Teredo and NAT64 ranges
- Simultaneous app instances share one latency probe cooldown; adopting a manifest spends at most about 9 s resolving sign-in hostnames

**Also fixed**

- Expired latency measurements are discarded when ranking servers, including after a failed refresh
- Best remains a latency/load heuristic; throughput and long-term reliability are unverified. [Remediation and runtime evidence](docs/STAFF-REMEDIATION.md)

## 0.1.2 · 2026-11-05 · early release

Easier installation. [Release notes](docs/RELEASE-v0.1.2.md).

**Added**

- Native packages: `.deb` for Ubuntu and Debian, `.rpm` for Fedora and openSUSE, `.pkg.tar.zst` for Arch and CachyOS, each pulling in its dependencies
- `sudo wave-vpn-uninstall` for tarball installs; no need to keep the download folder

**Changed**

- The tarball is self-contained: `README.txt`, `install.sh` and `LICENSE` on top, the installed files in `files/`. No separate source download needed to install
- `./install.sh` asks for your password itself, names missing dependencies for apt, dnf, pacman or zypper and offers to install them
- Network Lock is off by default; turn it on in Settings. Saved choices carry over. An enabled lock blocks the internet even while disconnected, which caught people out on first use
- Uninstalling disconnects and turns off Network Lock first instead of refusing
- Connecting from an uninstalled copy says Wave VPN isn't installed yet, instead of failing helper setup
- Installed documentation keeps the `docs/` folder, so the README and `docs/README.md` no longer overwrite each other

## 0.1.1 · 2026-11-03 · early release

New look; interface only. [Release notes](docs/RELEASE-v0.1.1.md).

**Changed**

- VesselWave family look shared with Wave Remote and both websites: blue-black dark theme, pill buttons, segments and filter chips, one corner scale

## 0.1.0 · 2026-11-02 · early release

First public build. [Release notes](docs/RELEASE-v0.1.0.md).

**Added**

- WireGuard client for AirVPN, written in Rust with Iced: sign-in, servers, Network Lock, status, logs and tray
- Secret Service account storage and read-only Eddie profile import
- Root-owned socket helper, firewall-state reconciliation, crash recovery, cancellable setup and retry backoff
- AirVPN IPv4, IPv6 and DNS checks; reviewed support report you can preview and export
- Source and binary release packaging with checksum and signature verification

**Status**

Authenticated and isolated network tests pass in the environments in [verification](docs/VERIFICATION.md).

**Not planned for now:** boot autoconnect, an advanced networking UI, other protocols and other desktop VPN backends.
