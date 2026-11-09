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
| `wave-tpl-ubuntu2604` | Ubuntu 26.04 LTS cloud image `release-20261016`, SHA-256 `5645d0318ad21f643ebb1ac4567514620e9893380b07b0201fda163baa5b7eb5` | see below |
| `wave-tpl-fedora45` | Fedora 45 Cloud Base Generic `45-1.4`, SHA-256 `50bdc5e70f6a24f540932d15411c1c6ae9fcfa40fb990d876d83c5cc823b2ad0` (signed `CHECKSUM` verified with the Fedora 45 key `4F50A6114CD5C6976A7F1179655A4B02F577861E`) | see below |

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
24.04 template user as "Ubuntu" (cloud-init default display name); later
templates set "Wave Lab".

`clean-desktop`: Proxmox does not allow snapshots of templates, so the sealed
template base is the clean state and every clone gets a `clean-desktop`
snapshot at creation (before its first boot). Rolling back to it gives a fresh
first boot with new SSH host keys and machine-id.

Login variants: autologin (default) and real GDM password login, switched with
`labctl login-mode NAME autologin|password --reboot`. `labctl gdm-login NAME`
types the private password into the greeter through QMP.

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

## Packages

`labctl build-packages SRC OUT` builds in the `wave-builder` linked clone
(Ubuntu 24.04 glibc baseline, rustup stable, nFPM 2.48.0 pinned with the
release workflow's checksum); the builder has a `toolchain` snapshot. First
build from `HEAD` `b2ca704`: rustc 1.99.1, `.deb`, `.rpm` and Arch packages with
`SHA256SUMS`, `SOURCE.txt` and `BUILD.json` in the private artifacts
directory. These are unsigned lab builds.

Upgrade baseline: the published v0.1.3 assets were downloaded with
`gh release download` and passed `scripts/verify-release.sh` (good signature,
trusted fingerprint `414018C0BBB8C0A70A80BF570AA32CCB2C0B3951`, all checksums).

## Host and limits

At most three test guests run at once (4 GiB, 2 vCPU each; builder 4 vCPU),
checked by `labctl start`. The network's apt cache only admits overlay-network
clients, so guests use the distribution mirrors directly; set `apt_proxy` again
once guests are allowed to reach a cache. When SSH is unreachable, the first
command waits for two 5-second connection attempts before using the agent;
later commands go straight to the agent for 30 seconds.

Not covered by this lab: hardware suspend/resume, physical GPUs, Secure Boot,
other display managers, and anything requiring a real AirVPN session unless a
test explicitly provides one.
