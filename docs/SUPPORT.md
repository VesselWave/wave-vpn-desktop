# Installation and support

Only Ubuntu 24.04 ARM64 (kernel 6.8, GNOME 46 on X11, local VM) has local networking evidence.
It is a development test environment, not a supported production release matrix.
Ubuntu x86_64 is a CI target; the workflow must actually run before claiming its
results. Other distributions, independent resolvconf implementations, Wayland
tray hosts and other architectures are unverified. macOS builds the UI only.
Windows, OpenVPN and AirVPN account-side port/DDNS management are unsupported.

## Build and install

From this repository root, with Rust and the distribution's development toolchain:

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked --bins
sudo bash scripts/install.sh install
wave-vpn-gui
```

On Ubuntu the runtime networking dependencies are `wireguard-tools iproute2
nftables` and systemd. A graphical polkit authentication agent and `pkexec`
are needed only for first-use setup when bypassing the installer.
The desktop automatically detects the DNS backend. Saved accounts and Eddie keyring
import require `libsecret-tools` and an unlocked desktop Secret Service.
The installer sets up a root-owned socket-activated networking helper and
authorizes the desktop user who invoked sudo. Future GUI connections and lock
actions need no password, including after reboot. This helper does not enable
boot-persistent Network Lock or boot autoconnect and never stores account credentials.
Run the same install command to upgrade. Existing preferences are retained;
quit the GUI before upgrading to use the new binaries. Both networking and
existing persistent-lock helper copies are updated on upgrade. Disconnect before
upgrading an active connection; installation refuses an active recovery journal.

```sh
sudo wave-vpn recover
# Explicitly restore ordinary internet access:
sudo wave-vpn lock-disable
# Only if you intentionally want to remove an installed persistent lock:
sudo wave-vpn lock-uninstall
sudo bash scripts/install.sh uninstall
```

The uninstaller refuses to remove binaries while a recovery journal or persistent
lock service exists. It preserves user preferences under `$XDG_CONFIG_HOME/wave-vpn`
(or `~/.config/wave-vpn`). It does not remove dependency packages or touch another
VPN manager's configuration. It removes Wave's networking helper and units.

## Diagnostics

Preview the allowlisted report before exporting or sharing:

```sh
wave-vpn diagnostics
wave-vpn diagnostics --output wave-diagnostics.json
```

Export creates a new mode-0600 file, refusing existing paths. The report omits
account names, secrets, addresses, preferences and raw logs. It is deliberately
minimal; do not attach private profiles or raw logs instead. Describe the action,
visible state, distribution/kernel, installed package versions and whether
Recover cleaned up the tunnel and explicit lock disable restored internet. Keep security reports private as described in
SECURITY.md.

## Reproducible local verification

Use a disposable Linux VM. Namespace tests require root and generated local keys,
not an AirVPN account. They isolate networking and /etc from the VM's host state.

```sh
bash scripts/verify-linux.sh
# After installation, as the authorized desktop user:
WAVE_SERVICE_TEST_VM=1 python3 scripts/test-service.py
# Full networking suite through the helper, in isolated namespaces:
sudo env WAVE_TEST_SERVICE=1 python3 tests/linux_network.py target/debug/wave-vpn
```

GNOME Secret Service and installed Eddie 2.26.2 CLI profile interoperability are
verified with synthetic credentials. Reproduce using an unlocked disposable VM:

```sh
bash scripts/test-keyring.sh
WAVE_EDDIE_DESKTOP_TEST=1 python3 scripts/test-eddie-import.py
```

The second test reads/writes only its own synthetic profile and keyring item.
It requires the official `eddie-ui` package at `/usr/lib/eddie-ui`. Its bundled
ARM64 GUI could not launch on this VM (requires libtiff.so.5); the shipped CLI
works and is used to save the real test profile. These import tests use synthetic credentials. Separate authenticated
backend verification passes for dual-stack AirVPN traffic, DNS, switching,
protected disconnect/crash/recovery and explicit unlock; see VERIFICATION.md.
GUI retry transactions, reboot/suspend/network changes and Wayland remain unverified. Boot autoconnect is outside the focused scope; ordinary Network Lock
does not survive reboot without the optional CLI service.

## Release candidates

`scripts/release.sh` requires Linux, a clean source checkout and `v<VERSION>` tag
at HEAD. It builds/tests and creates binary/source archives, dependency/toolchain
records and SHA-256 checksums under `target/distribution`. These are unsigned
candidates. No artifact should be advertised as production-ready until all parity
gates pass and the maintainer establishes a signing key, publishes its fingerprint
through an independently verifiable channel, signs the checksums (for example,
`gpg --armor --detach-sign <checksum-file>`) and verifies the signature from a
fresh environment. Release signing/private reporting contacts require maintainer
configuration; no signing identity or completed release is claimed.
