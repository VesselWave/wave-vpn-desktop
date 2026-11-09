# Desktop VM lab on the Proxmox host

2026-11-08. Disposable x86_64 desktop guests for the Linux test period, driven
from a workstation with `lab/labctl` (see `lab/README.md`). Host names,
addresses, VM identifiers, credentials and the private configuration stay in a
private directory outside Git. Guests live on the host's local NVMe directory
storage as qcow2 linked clones, in a dedicated VM ID range, named `wave-*` and
tagged `wave-lab`; `labctl` refuses to act on anything else.

## Templates

| Template | Base | State |
| --- | --- | --- |
| `wave-tpl-ubuntu2404` | Ubuntu 24.04.5 LTS cloud image `release-20261017`, SHA-256 `04e8f2461a16a44e78a370284471f29616cdd9baf1893df25807b71f985b6c94` (signed `SHA256SUMS` verified with the Ubuntu cloud image key `D2EB44626FDDC30B513D5BB71A5D6C4C7DB87C81`) | sealed, verified |
| `wave-tpl-ubuntu2604` | Ubuntu 26.04.1 LTS cloud image `release-20261016`, SHA-256 `5645d0318ad21f643ebb1ac4567514620e9893380b07b0201fda163baa5b7eb5` (from the same signed `SHA256SUMS` mechanism) | sealed, verified |
| `wave-tpl-fedora45` | Fedora 45 Cloud Base Generic `45-1.4`, SHA-256 `50bdc5e70f6a24f540932d15411c1c6ae9fcfa40fb990d876d83c5cc823b2ad0` (signed `CHECKSUM` verified with the Fedora 45 key `4F50A6114CD5C6976A7F1179655A4B02F577861E`), converted to Fedora Workstation 45 | sealed, verified |
| `wave-tpl-debian13` | Debian 13 `generic` cloud image `20261103-2651` (point release 13.7), SHA-512 `1207e6e3efaff8ee77ab7a32c156a3eea6a9e53f94cfc48efbd1f49ba554ca36ac62ab27395aa813a1775a8c0a393b1a1ce5377d9e5670f6113e3110c57fee4b` from `SHA512SUMS` over HTTPS (Debian publishes no signature for cloud images), GNOME | sealed, verified |
| `wave-tpl-opensuse-tw` | openSUSE Tumbleweed Minimal-VM Cloud `Snapshot20261107`, SHA-256 `caffa34a4111b77ab7bf99d5d1d51c83029e5e280f146c095c02bb11f95da734` (signed `.sha256.asc` verified with the openSUSE Project Signing Key `AD485664E901B867051AB15F35A2F86E29B700A4`), KDE Plasma | sealed, verified |
| `wave-tpl-arch` | Arch Linux cloud image `20261101.620452`, SHA-256 `794eb4889b39e1f0ed7d5ae998b2a1ed2137db8cf8c3476cf2962c7437ecb003` (signed `.SHA256.sig` verified with the arch-boxes key `1B9A16984A4E8CB448712D2AE0B78BF4326C6F8F`, signing subkey `656E4C5AC1CC3B86E539D97E343635A6859A9174`, as published in the arch-boxes README), KDE Plasma | sealed, verified |
| `wave-tpl-mint22` | Linux Mint 22.3 "Zena" Cinnamon live ISO, SHA-256 `a081ab202cfda17f6924128dbd2de8b63518ac0531bcfe3f1a1b88097c459bd4` (signed `sha256sum.txt.gpg` verified with the Linux Mint ISO Signing Key `27DEB15644C6B3CF3BD7D291300F846BA25BAE09`), installed by automatic Ubiquity | sealed, verified |

Ubuntu 24.04 template contents after provisioning: `ubuntu-desktop` 1.539.2
with GDM and GNOME on Wayland, kernel `linux-generic-hwe-24.04` 7.0.0-42 (the
flavour current point-release desktop media install) with netem available,
`ubuntu-appindicators@ubuntu.com` enabled, gnome-keyring as the Secret Service
provider, NetworkManager managing the NIC ("Wired connection 1") through the
installer's netplan stub, systemd-resolved stub mode, `systemd-networkd`
disabled and `systemd-networkd-wait-online` masked, qemu-guest-agent, serial
getty, persistent journal, 2 GiB `/swap.img`, and the test tools listed in the
README. No Wave VPN package and no Rust toolchain.

Documented deviations from a stock install: GDM autologin by default,
passwordless sudo for the test user, apt-daily/apt-daily-upgrade timers off and
unattended-upgrades removed, snap refreshes held, screen lock/idle blank off,
apport off, kernel console also on the serial port. The greeter shows the
24.04 template user as "Ubuntu" (cloud-init default display name); all other
templates show "Wave Lab". Fedora also makes only the guest agent's
SELinux domain permissive so agent fallback commands can inspect the session.

`clean-desktop`: Proxmox does not allow snapshots of templates, so the sealed
template base is the clean state and every clone gets a `clean-desktop`
snapshot at creation (before its first boot). Rolling back to it gives a fresh
first boot with new SSH host keys and machine-id.

Login variants: autologin (default) and real password login at the display
manager (GDM, SDDM or LightDM), switched with `labctl login-mode NAME
autologin|password --reboot`. `labctl dm-login NAME` (alias `gdm-login`) types
the private password into the greeter through QMP. `labctl wait NAME desktop`
waits for a GNOME, Plasma or Cinnamon session.

## Verification (Ubuntu 24.04, linked clone)

* Clone to GNOME Wayland session: 23 s from `start`; `systemctl
  is-system-running` = `running` (no failed units); hostname from the clone
  name; unique machine-id; NetworkManager connected.
* `screenshot` captured the desktop (1280x800 PNG), including with guest SSH
  blocked. `key ctrl-alt-t` opened a terminal, `type` entered a shell command
  whose output file was then read back, `click` on the top bar opened Quick
  Settings at the expected position.
* `run` over SSH; after an nft rule inside the guest dropped TCP/22, `run`
  fell back to the guest agent and returned the command's exit code (3) and
  stdout as the test user; `push`/`pull` round-tripped a file with the
  requested owner/mode through the agent.
* `rollback clean-desktop --start` removed the nft rule and the pushed file;
  SSH worked again against the regenerated, re-pinned host key.
* Password login: `login-mode password --reboot`, greeter screenshot,
  `gdm-login`: `gkr-pam: unlocked login keyring`, login collection
  `Locked=false`, `secret-tool store`/`lookup` without a prompt.
* Lab package smoke test: the `.deb` built below installed with `apt` on a
  clone (`libc6 (>= 2.39)` dependency satisfied); the clone was rolled back.
  No VPN connection was started anywhere.

## Verification (Ubuntu 26.04 and Fedora 45, linked clones)

* Ubuntu 26.04.1: kernel 7.0.0-43, `ubuntu-appindicators@ubuntu.com` enabled,
  NetworkManager + resolved stub, `systemd-networkd` disabled. The 26.04
  per-release "what's new" wizard is suppressed with its stamp file.
* Fedora 45 Workstation: kernel 7.2.15, SELinux enforcing, firewalld active in
  the `FedoraWorkstation` zone, NetworkManager + resolved stub, no extensions
  enabled (stock), gnome-tour suppressed.
* Both: clone booted to an active GNOME Wayland session with zero failed units
  and the clone hostname; desktop screenshots were clean; with TCP/22 dropped
  by nft inside the guest, `run` fell back to the agent (exit code 5 and stdout
  preserved, the agent could see `gnome-shell`), push/pull and screenshots
  worked; `rollback clean-desktop --start` removed the rule and the file and
  SSH worked again. Fedora password login through `gdm-login` produced a
  `gdm-password` PAM session with the login keyring unlocked and
  `secret-tool` store/lookup without a prompt.

## Verification (Debian 13, openSUSE Tumbleweed, Arch, Mint 22.3; linked clones)

The same run on each template: clone, boot to the graphical session, facts,
QMP `screenshot`/`key`/`click`, `run` over SSH, then an nft rule inside the
guest dropping TCP/22 so `run`, `push` and `pull` fall back to the guest agent,
`rollback clean-desktop --start`, password login with `login-mode password
--reboot` + `dm-login` and `secret-tool store`/`lookup`, then the matching lab
package from build `e194e24` installed on a fresh rollback. Clones were rolled
back and destroyed afterwards. No VPN connection was made.

| | Debian 13 | openSUSE TW | Arch | Mint 22.3 |
| --- | --- | --- | --- | --- |
| Clone to session | 21 s, GNOME Wayland (`gdm-autologin`) | 21 s, Plasma Wayland (`sddm-autologin`) | 25 s, Plasma Wayland | 22 s, Cinnamon X11 (LightDM) |
| Kernel | 6.12.116+deb13-amd64 | 7.3.1-1-default | 7.2.14-arch1-1 | 7.0.0-42-generic |
| System state | `running` | `running` | `running` | `degraded`: `casper-md5check.service` (stock, see below) |
| Agent fallback with SSH blocked | exit 5 and stdout kept, push/pull with owner/mode | same | same | same |
| Rollback | rule and file gone, SSH back | same | same | same |
| Password login, Secret Service | gnome-keyring 48.0; see first-login note | KWallet: `ksecretd --pam-login` (kwalletd6 6.31), `kdewallet` created with the login password; store/lookup without prompt | KWallet via pam_kwallet in SDDM; store/lookup without prompt | gnome-keyring, `gkr-pam: unlocked login keyring`; store/lookup without prompt |
| Lab package | `.deb` via apt: installed, dependencies satisfied | `.rpm` via zypper (`--allow-unsigned-rpm`): installed | `.pkg.tar.zst` via `pacman -U`: installed | `.deb` via apt: installed |

Each clone had the clone hostname, a unique machine-id, NetworkManager
connected on a predictably named NIC (`ens18`; Mint `enp6s18`), `sch_netem`
loadable, all test tools present and no Rust or Wave VPN.

Notes for testers:

* **Debian 13 DNS (D-12 case).** Debian's desktop install has no
  systemd-resolved: NetworkManager writes a plain `/etc/resolv.conf`
  (`# Generated by NetworkManager`), with no netplan or networkd and ifupdown
  configured for `lo` only. The template reproduces that from the cloud image,
  which ships netplan, networkd and resolved. No firewall is active (nftables is
  installed, its service is off); AppArmor is enabled.
* **Debian 13 first password login.** gnome-keyring 48.0 creates and unlocks
  `login.keyring` at the first password login (`gkr-pam: unlocked login
  keyring`), but that session's daemon only exports the `session` collection, so
  `secret-tool store` waits for a prompt that never appears. From the next login
  on (log out and in, or `systemctl restart gdm3` + `dm-login`), the `login`
  collection is exported and store/lookup work without a prompt. A fresh Debian
  user sees the same behaviour, so tests of first-run keyring handling should
  expect it.
* **openSUSE Tumbleweed.** NetworkManager (not wicked) writes a plain
  `/etc/resolv.conf`, no systemd-resolved. firewalld is active in the `public`
  zone with `ssh` and `dhcpv6-client`. SELinux is enforcing (only
  `virt_qemu_ga_t` permissive). The Secret Service provider is KWallet:
  `ksecretd`, started by pam_kwallet (`pam_kwallet5.so` in the common PAM
  stacks, package `pam_kwallet6`); gnome-keyring is not installed. With
  autologin the wallet is not unlocked by PAM. The guest agent's default RPC
  filter blocks guest-exec; the template clears it.
* **Arch.** NetworkManager feeds systemd-resolved (`/etc/resolv.conf` is the
  stub symlink); this is a lab choice, since Arch has no default and
  archinstall's NetworkManager profile leaves resolv.conf to NetworkManager.
  No firewall. KWallet through pam_kwallet in SDDM's PAM stack. Swap is zram plus
  the image's 512 MiB `/swap/swapfile`. There is no `hostname` command
  (inetutils is not installed).
* **Mint 22.3.** Cinnamon runs on X11 (`--session` still exports the bus and
  `DISPLAY=:0`). NetworkManager through the netplan stub with systemd-resolved,
  as on Ubuntu; ufw is installed but inactive. The installer leaves `casper`
  installed, so `casper-md5check.service` fails at every boot and the system
  reports `degraded`; ignore that unit when checking for failures. The template
  is installed with Mint's own Ubiquity from the signed ISO, not converted from
  Ubuntu; "Install multimedia codecs" was left unticked (installer default).
* **SDDM greeters** run on X11 even though the Plasma sessions are Wayland.

Build issues found and fixed while building these templates: openSUSE's
cloud-init prints no host keys (now scanned from the host) and its Minimal-VM
image skips recommended packages (masked before installing the desktop
patterns); Arch's cloud image waited for NTP before starting sshd and re-ran
its keyring init on every first boot (both disabled); Proxmox only attaches ISO
files from ISO-capable storage, so the Mint installer ISO is attached through
VM `args` for the install and removed afterwards.

## Packages

`labctl build-packages SRC OUT` builds in the `wave-builder` linked clone
(Ubuntu 24.04 glibc baseline, rustup stable, nFPM 2.48.0 pinned with the
release workflow's checksum); the builder has a `toolchain` snapshot. First
build from `HEAD` `b2ca704`: rustc 1.99.1, `.deb`, `.rpm` and Arch packages with
`SHA256SUMS`, `SOURCE.txt` and `BUILD.json` in the private artifacts
directory. A rebuild with the existing builder (new `HEAD`, warm Cargo
target) took 70 seconds end to end. These are unsigned lab builds.

Upgrade baseline: the published v0.1.3 assets were downloaded with
`gh release download` and passed `scripts/verify-release.sh` (good signature,
trusted fingerprint `414018C0BBB8C0A70A80BF570AA32CCB2C0B3951`, all checksums).

Storage after the 2026-11-09 templates: Debian 5.2 GiB, openSUSE 6.5 GiB,
Arch 3.6 GiB, Mint 9.0 GiB on local storage. About 14.5 GiB remained free with
all clones destroyed and the downloaded images and ISO removed (they are
fetched again on rebuild). Do not add more templates without freeing space.

Observed host load: about 35% IO pressure (`some`, 10 s average) while two
templates installed packages concurrently, under 3% when idle; memory stayed
above 11 GiB available. The three sealed templates use about 22 GiB of the
local storage and the builder about 3 GiB.

## Host and limits

At most three test guests run at once (4 GiB, 2 vCPU each; builder 4 vCPU),
checked by `labctl start`. The network's apt cache only admits overlay-network
clients, so guests use the distribution mirrors directly; set `apt_proxy` again
once guests are allowed to reach a cache. When SSH is unreachable, the first
command waits for two 5-second connection attempts before using the agent;
later commands go straight to the agent for 30 seconds.

Not covered by this lab: hardware suspend/resume, physical GPUs, Secure Boot,
display managers other than GDM, SDDM and LightDM, desktops other than GNOME,
Plasma and Cinnamon, and anything requiring a real AirVPN session unless a test
explicitly provides one.
