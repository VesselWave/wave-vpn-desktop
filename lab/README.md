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

## x86_64 guests

Run `proxmox/create-ubuntu.sh PUBLIC_KEY_FILE` on an authorized Proxmox lab host.
It validates the public key and pinned image, allocates a free VM identifier,
and provisions an Ubuntu GNOME guest with the example account `wave-lab`.
Generated login credentials and cloud-init inputs stay private on the lab host;
SSH password authentication is disabled. Adapt storage names to your own host.

Run `proxmox/enable-gnome-rdp.sh` as root inside that dedicated guest only if RDP
is needed. It generates its TLS certificate and RDP password locally. Access RDP
through an SSH tunnel configured with your own private host/user settings.
Never publish actual management endpoints or generated credentials.

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
