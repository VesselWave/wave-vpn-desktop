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
lab/labctl template-create ubuntu2404          # build + seal a template (once)
lab/labctl clone wave-tpl-ubuntu2404 wave-u1   # linked clone + "clean-desktop" snapshot
lab/labctl start wave-u1 --wait gnome
lab/labctl run wave-u1 'systemctl is-system-running'
lab/labctl run wave-u1 --root --json -- nft list ruleset
lab/labctl run wave-u1 --session 'gsettings get org.gnome.desktop.interface color-scheme'
lab/labctl push wave-u1 ./wave-vpn_0.1.3-1_amd64.deb /tmp/wave.deb
lab/labctl pull wave-u1 /tmp/evidence.pcap ./evidence.pcap --root
lab/labctl screenshot wave-u1 desktop.png
lab/labctl key wave-u1 super            # also ctrl-alt-t, ret, esc, f10 ...
lab/labctl type wave-u1 'wave-vpn-gui'
lab/labctl click wave-u1 640 400 [--button right] [--double]
lab/labctl wait wave-u1 agent|ip|ssh|greeter|session|gnome [--timeout 300]
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
  `gnome-shell` running; `greeter` waits for the GDM greeter session.
* **login-mode NAME autologin|password [--reboot]** switches GDM.
  **gdm-login NAME** waits for the greeter and types the private password, which
  exercises PAM and unlocks the login keyring. Typical PAM keyring run:
  `clone` → `start` → `login-mode password --reboot` → `gdm-login`. Do the first
  graphical login with the password so the login keyring gets that password.
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

`template-create ubuntu2404|ubuntu2604|fedora45` downloads the pinned Ubuntu cloud image to the
host's `image_dir`, checks its SHA-256, creates the VM on the configured storage
(q35, OVMF without enrolled keys, virtio-scsi qcow2, virtio-gpu, USB tablet,
`serial0` socket, guest agent), boots it once with Proxmox cloud-init (key-only
login), reads the SSH host key (and, when printed, the address) from the serial
console, otherwise finds the address by MAC in the host's neighbour table, then
runs `templates/ubuntu-provision.sh install` (or `fedora-provision.sh`) as a
transient systemd unit. The Ubuntu script provides:

* `ubuntu-desktop` with GDM and GNOME on Wayland, the Ubuntu AppIndicators
  extension, gnome-keyring, the HWE kernel on 24.04 (as installed by current
  point-release media) with `linux-modules-extra` (netem);
* NetworkManager for every device through the installer's netplan stub
  (`renderer: NetworkManager`), systemd-resolved, `systemd-networkd` and its
  `wait-online` unit disabled/masked, cloud-init disabled afterwards;
* the `wave-lab` user in the usual desktop groups with a generated password;
* qemu-guest-agent, a serial getty on `ttyS0`, a persistent journal, a 2 GiB
  `/swap.img`;
* test tools: tcpdump, tshark, nftables, wireguard-tools, dnsutils, curl,
  conntrack, jq, iproute2, libsecret-tools, python3. No Wave VPN, no Rust.

It reboots, waits for the GNOME Wayland session, saves a screenshot to the
private artifacts directory, runs `seal` (removes SSH host keys, machine-id,
DHCP leases, keyrings, logs and cloud-init state, trims the disk), powers off and
converts the VM to a template. Each clone's first boot generates SSH host keys
and takes its hostname from the SMBIOS serial that `clone` sets.

`fedora-provision.sh` installs `workstation-product-environment` on Fedora
Cloud Base, switches the release identity to Workstation (os-release, presets
and the `FedoraWorkstation` firewalld zone), keeps SELinux enforcing and
firewalld active, enables sshd, the guest agent and a serial getty, and installs
the same test tools (`wireshark-cli`, `bind-utils`, `conntrack-tools`,
`iproute-tc`, `kernel-modules-extra`, `libsecret`). The AppIndicator extension is
installed but, as on stock Fedora, not enabled.

If provisioning succeeds but a later step fails, fix the guest and resume with
`labctl template-seal NAME DISTRO` instead of starting over.

Deliberate deviations from a stock desktop install, for unattended testing:
GDM autologin by default (switch with `login-mode`); passwordless `sudo` for the
test user; apt-daily, apt-daily-upgrade and unattended-upgrades disabled and
snap refreshes held, so the system does not change under a test; screen lock and
idle blanking disabled and apport crash dialogs off; the kernel console also on
`ttyS0`; first-login welcome/tour dialogs suppressed. Fedora additionally makes
only the `virt_qemu_ga_t` SELinux domain permissive (guest-exec children would
otherwise be confined and unable to inspect the session), clears the guest-agent
RPC filter, and disables `dnf-makecache.timer` and GNOME Software downloads.
Re-enable any of these inside a clone when a test needs them.

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
