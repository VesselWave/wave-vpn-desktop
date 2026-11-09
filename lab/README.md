# Desktop verification lab

Provision disposable Linux guests for desktop, helper, keyring and network tests.
Keep real hostnames, IP addresses, VM identifiers, credentials, boot identifiers
and access instructions in private local configuration.

## ARM64 guests

```sh
limactl start -y --name wave-fedora-kde lab/lima/fedora-kde.yaml
limactl start -y --name wave-debian-xfce lab/lima/debian-xfce.yaml
limactl shell wave-fedora-kde
limactl shell wave-debian-xfce
```

The configurations pin distribution images/checksums, use normal EFI firmware,
mount no host directories, and install desktop/build/network prerequisites.
They configure automatic desktop login with a generated local password stored
privately inside each disposable guest. Automatic login is not proof of PAM or
keyring unlock. SSH uses keys. The optional pinned public dotfiles checkout is
guest configuration, not incorporated into application source.

## x86_64 guests on Proxmox VE: `labctl`

`labctl` (Python 3 standard library only) drives disposable guests on a Proxmox
VE host from a workstation over SSH. It reads a private JSON configuration from
`$WAVE_LAB_CONFIG`, else the nearest `.lab/config.json` above the current or
`lab/` directory. Keep that directory outside every Git repository, mode 0700:

```json
{
  "host": "root@PROXMOX_HOST",
  "storage": "local",
  "vmid_min": 900, "vmid_max": 949, "clone_vmid_min": 910,
  "tag": "wave-lab", "name_prefix": "wave-", "max_running": 3,
  "guest_user": "wave-lab",
  "ssh_key": "ssh/id_ed25519",
  "password_file": "secrets/wave-lab-password",
  "apt_proxy": null,
  "memory_mb": 4096, "cores": 2,
  "builder": {"name": "wave-builder", "template": "wave-tpl-ubuntu2404", "cores": 4, "memory_mb": 4096}
}
```

`host`, `ssh_key` and the VM ID range are required. Relative paths resolve
against the configuration directory. `labctl` only acts
on QEMU guests whose ID is in the configured range, whose name has the prefix
and that carry the tag; anything else is refused. Templates are destroyed only
with `--template`. `start` refuses to exceed `max_running` without `--force`.
Cached guest addresses and pinned SSH host keys live in `.lab/state/`.

```sh
lab/labctl list
lab/labctl template-create ubuntu2404          # build + seal a template (once; see Templates)
lab/labctl clone wave-tpl-ubuntu2404 wave-u1   # linked clone + "clean-desktop" snapshot
lab/labctl start wave-u1 --wait desktop        # or --wait gnome on GNOME guests
lab/labctl run wave-u1 'systemctl is-system-running'
lab/labctl run wave-u1 --root --json -- nft list ruleset
lab/labctl run wave-u1 --session 'gsettings get org.gnome.desktop.interface color-scheme'
lab/labctl push wave-u1 ./wave-vpn_0.1.3-1_amd64.deb /tmp/wave.deb
lab/labctl pull wave-u1 /tmp/evidence.pcap ./evidence.pcap --root
lab/labctl screenshot wave-u1 desktop.png
lab/labctl key wave-u1 super            # also ctrl-alt-t, ret, esc, f10 ...
lab/labctl type wave-u1 'wave-vpn-gui'
lab/labctl click wave-u1 640 400 [--button right] [--double]
lab/labctl wait wave-u1 agent|ip|ssh|greeter|session|gnome|desktop [--timeout 300]
lab/labctl login-mode wave-u1 password --reboot && lab/labctl dm-login wave-u1
lab/labctl snapshot wave-u1 before-install [--vmstate]
lab/labctl rollback wave-u1 clean-desktop [--start]
lab/labctl snapshots wave-u1
lab/labctl serial wave-u1 --send '\r' --read 3    # or: labctl console wave-u1
lab/labctl ssh wave-u1
lab/labctl stop wave-u1 [--hard]
lab/labctl destroy wave-u1
```

* **run** uses SSH as the guest user (`--root` adds `sudo -n`, `--session`
  exports the GNOME session's `XDG_RUNTIME_DIR`, D-Bus and Wayland variables).
  When SSH cannot connect at all, for example because Network Lock blocks the
  LAN, it falls back to `guest-exec` through the QEMU guest agent and keeps the
  exit code, stdout and stderr (`--json` reports which path was used). A
  connection that drops after the command started is reported as exit 255 and
  never re-run. `--via ssh|agent` forces one path. Quote the command or put it
  after `--` when it has options of its own.
* **push/pull** stream files over SSH, falling back to chunked guest-agent file
  reads and writes. Copy directories as tarballs.
* **screenshot, key, type, click, move, scroll** use the host's QMP socket
  (`screendump`, `send-key`, `input-send-event` on the USB tablet), so they work
  on GNOME Wayland, at the greeter and with guest networking blocked. Pixel
  coordinates refer to the screenshot. `type` uses a US keyboard map.
* **wait gnome** requires an active Wayland session of the guest user with
  `gnome-shell` running. **wait desktop** accepts any active Wayland or X11
  session of the guest user on the current VT running `gnome-shell`,
  `plasmashell` or `cinnamon`. `greeter` waits for the display manager's greeter
  session (GDM, SDDM or LightDM).
* **login-mode NAME autologin|password [--reboot]** switches the display
  manager it finds: GDM (`/etc/gdm3/daemon.conf` on Debian, `custom.conf`
  elsewhere), SDDM (`/etc/sddm.conf.d/zz-wave-lab-autologin.conf`, plus
  `DISPLAYMANAGER_AUTOLOGIN` on openSUSE) or LightDM
  (`/etc/lightdm/lightdm.conf.d/90-wave-lab-autologin.conf`).
  **dm-login NAME** (alias `gdm-login`) waits for the greeter and types the
  private password, which exercises PAM and unlocks the login keyring or
  KWallet. Typical PAM keyring run: `clone` → `start` → `login-mode password
  --reboot` → `dm-login`. Do the first graphical login with the password so the
  keyring gets that password (Debian 13: see the lab notes, the first session
  does not export it yet).
* **build-packages SRC OUT** builds `.deb`, `.rpm` and Arch packages outside the
  test guests, in a `wave-builder` linked clone of the Ubuntu 24.04 template
  (created and set up on first use with `builder/setup-ubuntu.sh`: build tools,
  rustup stable and the nFPM version pinned by the release workflow). The source
  is `git archive` of `--ref` (default `HEAD`), or with `--worktree` the tracked
  files as they are in the working tree; Git metadata, ignored and untracked
  files never leave the workstation. The guest runs `builder/build.sh`, which
  wraps `cargo build --release --locked --bins` and `scripts/package.sh`
  (`--checks` adds fmt/test/clippy). Results land in
  `OUT/wave-vpn-VERSION-REVISION/` with `SHA256SUMS`, `SOURCE.txt` and
  `BUILD.json`. The builder stops afterwards unless `--keep-running`; its Cargo
  target directory persists for incremental rebuilds. Lab packages are unsigned
  test builds, not release candidates.

### Templates

`template-create DISTRO` builds and seals one template. Cloud-image distros
(`ubuntu2404`, `ubuntu2604`, `fedora45`, `debian13`, `opensuse-tw`, `arch`)
download the pinned image to the host's `image_dir`, check its SHA-256 or
SHA-512, create the VM on the configured storage (q35, OVMF without enrolled
keys, virtio-scsi qcow2, virtio-gpu, USB tablet, `serial0` socket, guest agent),
boot it once with Proxmox cloud-init (key-only login), read the SSH host key
(and, when printed, the address) from the serial console, otherwise find the
address by MAC in the host's neighbour table. Images that print no host key
(openSUSE) are pinned with `ssh-keyscan` from the Proxmox host on first use. It
then runs `templates/DISTRO-provision.sh install` as a transient systemd unit,
reboots, waits for the desktop session, saves a screenshot to the private
artifacts directory, runs `seal` (removes SSH host keys, machine-id, DHCP
leases, keyrings/wallets, logs and cloud-init state, trims the disk), powers off
and converts the VM to a template. Each clone's first boot generates SSH host
keys and takes its hostname from the SMBIOS serial that `clone` sets.

`mint22` has no cloud image. labctl installs the signed live ISO with the
distribution's own installer: it extracts the casper kernel and initrd on the
host, appends a filled copy of `templates/mint-preseed.cfg` (crypted password,
lab public key) to the initrd as an extra cpio, boots the kernel directly with
`automatic-ubiquity` and the ISO attached read-only, and waits for Ubiquity to
power off. Ubiquity erases the disk with its default layout, creates the user
with autologin and, in its `success_command`, installs openssh-server and the
guest agent and authorises the lab key. The working files and the VM `args` are
removed, the installed system boots, and `mint-provision.sh` adds the lab
tools as on the other templates. The desktop is exactly what the installer put
there. The host needs `isoinfo` and `openssl`.

| Template | Desktop and display manager | Network and DNS | Firewall | Secret Service |
| --- | --- | --- | --- | --- |
| `wave-tpl-ubuntu2404`, `-ubuntu2604` | `ubuntu-desktop`, GNOME Wayland, GDM | NetworkManager via netplan stub, systemd-resolved stub | ufw not installed | gnome-keyring |
| `wave-tpl-fedora45` | Workstation, GNOME Wayland, GDM | NetworkManager, systemd-resolved stub | firewalld `FedoraWorkstation` | gnome-keyring |
| `wave-tpl-debian13` | tasks `desktop` + `gnome-desktop`, GNOME Wayland, GDM | NetworkManager writes a plain `/etc/resolv.conf`; no resolved, netplan or networkd; ifupdown knows only `lo` | none active (nftables installed, service off) | gnome-keyring |
| `wave-tpl-opensuse-tw` | patterns `kde kde_plasma x11 enhanced_base`, Plasma Wayland, SDDM | NetworkManager writes a plain `/etc/resolv.conf`; no resolved, no wicked | firewalld `public` (ssh, dhcpv6-client) | KWallet (`ksecretd` from kwalletd6, unlocked by pam_kwallet) |
| `wave-tpl-arch` | archinstall KDE profile set, Plasma Wayland, SDDM | NetworkManager + systemd-resolved stub | none | KWallet (pam_kwallet in SDDM) |
| `wave-tpl-mint22` | Linux Mint 22.3 Cinnamon (X11), LightDM slick-greeter | NetworkManager via netplan stub, systemd-resolved stub | ufw installed, inactive | gnome-keyring |

Every template has the `wave-lab` user ("Wave Lab") in the distribution's usual
desktop groups with the private password, qemu-guest-agent with guest-exec, a
serial getty on `ttyS0`, a persistent journal, the test tools (tcpdump, tshark,
nftables, wireguard-tools, dig, curl, conntrack, jq, iproute2 with `tc` and
`sch_netem`, `secret-tool`, python3), no Wave VPN and no Rust.

Per-distro provisioning notes:

* `ubuntu-provision.sh`: `ubuntu-desktop`, the Ubuntu AppIndicators extension,
  the HWE kernel on 24.04 (as installed by current point-release media) with
  `linux-modules-extra`; NetworkManager for every device through the
  installer's netplan stub (`renderer: NetworkManager`), `systemd-networkd` and
  its `wait-online` unit disabled/masked, cloud-init disabled; 2 GiB `/swap.img`.
* `fedora-provision.sh`: `workstation-product-environment` on Fedora Cloud
  Base, release identity switched to Workstation (os-release, presets and the
  `FedoraWorkstation` firewalld zone), SELinux enforcing, firewalld active. The
  AppIndicator extension is installed but, as on stock Fedora, not enabled.
* `debian-provision.sh`: the `generic` image (standard kernel with DRM and
  netem, not the `genericcloud` cloud kernel), tasksel's `task-desktop` and
  `task-gnome-desktop` with Recommends, then the d-i desktop network layout:
  netplan, systemd-networkd, systemd-resolved and cloud-init removed,
  `/etc/network/interfaces` with only `lo`, NetworkManager writing
  `/etc/resolv.conf`; 2 GiB `/swapfile`. No AppIndicator extension (stock).
* `opensuse-provision.sh`: Minimal-VM ships zypp drop-ins that skip recommended
  packages and docs; same-named files in `/etc/zypp/zypp.conf.d` restore the
  installed-system defaults before the patterns, and `install-new-recommends`
  completes earlier installs. `kernel-default` replaces `kernel-default-base`.
  SDDM runs through `display-manager-legacy.service` and the
  `default-displaymanager` alternative. SELinux stays enforcing. cloud-init
  removed; 2 GiB `/swapfile`.
* `arch-provision.sh`: `plasma-meta`, konsole, kwrite, dolphin, ark, sddm,
  PipeWire, NetworkManager, `en_US.UTF-8`, zram swap (archinstall defaults;
  the image's 512 MiB btrfs swap file stays). Arch has no default resolver
  stack: systemd-resolved (already enabled in the image) is kept in stub mode
  and NetworkManager feeds it, so Arch covers Plasma with resolved while
  openSUSE covers Plasma without it. Cloud-image boot settings are removed:
  `net.ifnames=0`, `systemd-time-wait-sync` (held sshd until NTP synced) and
  the per-boot `pacman-init`. cloud-init removed.
* `mint-provision.sh`: full upgrade (as the Update Manager would do once), the
  test tools, LightDM autologin drop-in. The installer leaves `casper`
  installed, so `casper-md5check.service` fails on every boot and
  `systemctl is-system-running` reports `degraded`; this is the stock state.

If provisioning succeeds but a later step fails, fix the guest and resume with
`labctl template-seal NAME DISTRO` instead of starting over.

Deliberate deviations from a stock desktop install, for unattended testing:
display-manager autologin by default (switch with `login-mode`); passwordless
`sudo` for the test user; no background package activity (apt-daily timers and
unattended-upgrades off, snap refreshes held, `dnf-makecache.timer` and GNOME
Software downloads off, Discover's update notifier and Mint's Update Manager
not autostarted, Mint's automation timers off); screen lock, idle blanking and
idle suspend disabled (dconf on GNOME/Cinnamon, `kscreenlockerrc` and
`powerdevilrc` in `/etc/xdg` on Plasma) and apport crash dialogs off; the kernel
console also on `ttyS0`; first-login welcome dialogs suppressed with each
desktop's own stamp (`gnome-initial-setup-done`, Plasma Welcome Center's
`LastSeenVersion`, the openSUSE welcome launcher's `launched` file, Mint's
`mintwelcome/norun.flag`). Fedora and openSUSE make only the `virt_qemu_ga_t`
SELinux domain permissive (guest-exec children would otherwise be confined and
unable to inspect the session) and clear the guest agent's RPC filter, which
blocks guest-exec by default on both. Debian/openSUSE use a swap file where the
installer would make a partition; openSUSE's root is the image's XFS, not the
installer's btrfs with snapper. Re-enable any of these inside a clone when a
test needs them.

When changing a template image pin, verify the distribution's published
signature where there is one and record it in the lab notes.
Tumbleweed and Arch mirrors drop old snapshots, so update those pins (URL and
checksum) before rebuilding.

### Legacy scripts

`proxmox/create-ubuntu.sh` and `proxmox/enable-gnome-rdp.sh` provision the older
single-guest setup (storage names are site-specific). Run `enable-gnome-rdp.sh`
as root inside a dedicated guest only if RDP is needed. It generates its TLS
certificate and RDP password locally. Never publish management endpoints or
generated credentials.

### Tests

```sh
python3 -m unittest discover -s lab/tests -v
```

They stub `ssh` on `PATH` (fake Proxmox host and guest) and run the host helper
against fake QMP/QGA sockets; nothing contacts a real host.

## Qualification

Verify desktop packages, the display manager, an actual graphical session,
Secret Service, polkit and a changed boot session after reboot. Debian requires
the graphics-capable standard kernel; Fedora KDE uses its configured display
manager. Neither an installed environment nor automatic login qualifies the app.

Copy only tracked source into a guest; omit Git metadata, ignored files, private
configuration and build outputs. Run `bash lab/bootstrap-rust.sh` as the guest
user, then the checks in CONTRIBUTING.md. Low-memory guests can use
`CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0`.

Physical suspend/resume remains a separate real-machine test. Guest pause/sleep,
synthetic screenshots and isolated namespace tests do not establish hardware,
authenticated GUI, manual PAM or production VPN qualification.
