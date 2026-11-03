# Installation and support

Ubuntu 24.04 ARM64 GNOME X11 and x86_64 GNOME Wayland have local networking
and desktop evidence. The x86_64 candidate also has helper-security and normal
EFI persistent-lock boot checks. Debian ARM64 Xfce and Fedora ARM64 KDE have
separate namespace-test evidence; this does not establish complete GUI support.
See VERIFICATION.md for exact binary hashes, failures and remaining gates.
This is a development test matrix, not a supported production release promise.
Hosted CI and the tagged v0.1.0 packaging workflow passed; these checks do not
qualify the complete GUI or physical lifecycle. Other
distributions, independent resolvconf implementations and other architectures
remain unverified. macOS builds the UI only.
Windows, OpenVPN and AirVPN account-side port/DDNS management are unsupported.

## Install the v0.1.0 prebuilt candidate

Read [the known limits](RELEASE-v0.1.0.md) first. Download both source and binary
archives plus the signed manifest and key from the same release. Verify them
using the independently trusted fingerprint and `scripts/verify-release.sh`
from a trusted source checkout before extracting or executing anything.
The published binary is x86_64 Linux; it does not run on ARM64 or macOS.

After installing the runtime dependencies described below, extract in a new
empty directory and install using the source archive's installer:

```sh
tar -xzf wave-vpn-0.1.0-source.tar.gz
tar -xzf wave-vpn-0.1.0-x86_64-unknown-linux-gnu.tar.gz
sudo bash wave-vpn-0.1.0/scripts/install.sh install \
  "$PWD/wave-vpn-0.1.0-x86_64-unknown-linux-gnu/usr/local/bin"
wave-vpn-gui
```

Rust is not required for this prebuilt installation. Disconnect and quit Wave
before upgrading. Retain the source directory for the documented uninstaller.

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

GNOME Secret Service and installed Eddie 2.26.2 / 2.27.2 CLI profile interoperability are
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
Final-candidate GUI/account/error coverage, manual PAM keyring unlock and physical
suspend still need verification. VM reboot/helper checks do not establish physical
sleep behavior. Boot autoconnect is outside the focused scope; ordinary Network Lock
does not survive reboot without the optional CLI service.

## Release candidates

`scripts/release.sh` requires Linux, a clean source checkout and `v<VERSION>` tag
at HEAD. It builds/tests and creates binary/source archives, dependency/toolchain
records and SHA-256 checksums under `target/distribution`. These are unsigned
candidates. No artifact should be advertised as production-ready until all parity
gates pass. The maintainer signing identity and private reporting contact are
recorded in SECURITY.md. Publish the fingerprint through a separately trusted
channel and confirm inbox delivery and signing-key backup before distribution.
Use `bash scripts/release.sh --sign-key FULL_FINGERPRINT` for a signed candidate,
then independently verify it with `scripts/verify-release.sh`. The [v0.1.0 experimental prerelease](https://github.com/VesselWave/wave-vpn-desktop/releases/tag/v0.1.0)
is published in the currently access-restricted repository. Public accessibility
and inbox delivery remain separate unconfirmed gates.

## Sign-in fails after a broken VPN session

An account refusal and an unreachable/invalid bootstrap response have different
messages. A network failure does not mean your password was wrong. Use Overview
→ Recover even when signed out, then retry. An enabled Network Lock is retained;
use the separate Network Lock control only when you intend to unlock internet.
A new connection also repairs a leftover Wave journal after a worker crash.
If another VPN owns the stale DNS or firewall state, resolve it in that VPN.
Do not flush the system firewall or delete recovery records as a general remedy.

The status query reads nftables through the authorized helper; missing helper,
query errors and foreign tables produce unknown status. A missing/stale cached
JSON file is never taken as proof that protection is on or off. Reopening Wave
shows whether recovery is needed and whether a worker is still active.

For support, use Logs → Support report, review it, then copy or save. The report
contains no raw logs, addresses or account material. A separate export failure
never overwrites an existing report. Include OS/version and reproduction steps;
do not publish profiles or keyring contents. See RELEASE-GATES.md for release
signature checking and the remaining release gates.
