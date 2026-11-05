# Contributing

Bug reports and pull requests are welcome. For security issues, email **vesselwave@protonmail.com** instead of opening an issue ([SECURITY.md](SECURITY.md)).

## Build and check

Needs a recent stable Rust. VPN connections need Linux with kernel WireGuard, `wireguard-tools`, `iproute2`, `nftables`, `getent` and systemd; the UI also builds on macOS.

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked --bins
./scripts/install.sh            # installs under /usr/local; asks for sudo
wave-vpn-gui
```

`scripts/package.sh OUTPUT_DIR` builds the .deb, .rpm and Arch packages from the release binaries with [nFPM](https://nfpm.goreleaser.com) (`NFPM=/path/to/nfpm` if it isn't on PATH). `scripts/release.sh` builds the whole release: the tarball (`README.txt`, `install.sh`, `files/`), the packages, the source archive and checksums. Package install and removal hooks are in `packaging/scripts/`.

`WAVE_PREVIEW=1 wave-vpn-gui` starts the UI with sample data and no networking; add `WAVE_PREVIEW_SCENE=connected` (or `diagnostics`, `unknown`, `logged-out`, `error`) and `WAVE_CAPTURE_DIR=…` to save screenshots.

## Network tests

Run the isolated networking suite **inside a Linux test VM**. It creates private mount, network and PID namespaces with an `/etc` overlay, needs no AirVPN account or internet, and leaves the host's resolver and firewall untouched:

```sh
cargo build --bins
sudo python3 tests/linux_network.py target/debug/wave-vpn
sudo env WAVE_TEST_SERVICE=1 python3 tests/linux_network.py target/debug/wave-vpn
```

The authenticated live suite signs in on the host and passes only the generated WireGuard profiles to a disposable Lima VM through stdin:

```sh
python3 scripts/verify-airvpn.py --vm VM_NAME --guest-binary /absolute/path/to/wave-vpn
```

It reads `KEY=<JSON string>` credentials from a private `.env` (mode 0600). Lab VM setup is in [lab/](lab/README.md).

### Helper, keyring and Eddie checks

In a disposable VM, after installing:

```sh
bash scripts/verify-linux.sh
WAVE_SERVICE_TEST_VM=1 python3 scripts/test-service.py      # as the authorized desktop user
bash scripts/test-keyring.sh                                # needs an unlocked GNOME keyring
WAVE_EDDIE_DESKTOP_TEST=1 python3 scripts/test-eddie-import.py
```

The keyring and Eddie tests use synthetic credentials and touch only their own profile and keyring item. The Eddie test needs the official `eddie-ui` package at `/usr/lib/eddie-ui`; it has been verified against Eddie 2.26.2 and 2.27.2 CLI profiles. (On the ARM64 test VM, Eddie's bundled GUI couldn't start because it needs `libtiff.so.5`, so its CLI saves the test profile.)

## How it fits together

- `wave-vpn-gui` (Iced) runs as your user and owns the UI, account and keyring.
- `wave-vpn` is the CLI and, under a root-owned socket-activated systemd unit, the networking helper. The helper checks the caller's kernel peer credentials against the installing user and starts a worker for a fixed set of actions.
- The worker validates structured WireGuard profile and option data from a pipe, configures its own interface, policy routes and nftables table, and journals everything to a root-only recovery record. It never runs shell commands or raw config directives.

## Where code may come from

- Protocol behavior follows the pinned official Eddie sources in `protocol-data/` and other explicitly licensed references. Record any new reference or reuse in [docs/PROVENANCE.md](docs/PROVENANCE.md).
- Never commit credentials, private profiles, signing keys, local paths or build outputs.
- The project is GPL-3.0-or-later; keep dependency notices intact.

Releases are built, signed and published by CI when a `v*` tag is pushed; the steps and gates are in [docs/RELEASE-GATES.md](docs/RELEASE-GATES.md).
