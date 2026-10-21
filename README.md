# Wave VPN for Linux

Unofficial AirVPN desktop client, built with Rust and Iced. GPLv3 or later;
not affiliated with AirVPN.

This repository builds independently. See [provenance](docs/PROVENANCE.md) for
pinned Eddie references and authored presentation assets.

## Build and install

Requires Rust. Connections currently require AirVPN Hummingbird 2.x and its
runtime dependencies, plus `pkexec` with a running authentication agent.

```sh
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo build --release --locked
sudo install -m 755 target/release/wave-vpn target/release/wave-vpn-gui /usr/local/bin/
wave-vpn-gui
```

The GUI runs as your user. A privileged worker validates structured WireGuard
profile data received through a pipe and invokes root-owned Hummingbird with
Network Lock enabled. Passwords are prompted privately or entered in the GUI;
account credentials are kept in memory rather than saved by this app.

Generate a private configuration for another VPN manager:

```sh
wave-vpn profile --login YOUR_USERNAME --server SERVER_NAME --output wave.conf
```

The new mode-0600 output file contains private keys. Keep it private.

Optional live bootstrap test, using a synthetic invalid account:

```sh
cargo test --test protocol live_bootstrap -- --ignored
```

## Status

New protocol implementation, controller and views with Wave's design and authored
icons/fonts. Tray support, traffic statistics, persistent preferences and country
grouping still need restoration. Authenticated connections, Hummingbird integration
and leak/firewall behavior require Linux runtime testing before release.

[Verification record](docs/VERIFICATION.md). This is an unpublished development build.

## Published history privacy

The repository history and release packages were sanitized on 2026-11-03.
Lab records use example identities and addresses; private management details
are omitted. Release archives and signed checksums were repackaged while keeping
the APK and executable bytes unchanged. Existing clones may retain old history.
