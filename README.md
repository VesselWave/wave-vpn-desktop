# Wave VPN for Linux

Unofficial AirVPN desktop client, built with Rust and Iced. GPLv3 or later;
not affiliated with AirVPN.

This repository builds independently. See [provenance](docs/PROVENANCE.md) for
pinned Eddie references and authored presentation assets.

## Build and install

Requires Rust. Linux connections require kernel WireGuard, `wireguard-tools`,
`iproute2`, `nftables`, `getent` (Ubuntu libc-bin) and systemd. A bare binary installation also needs
`pkexec` with an authentication agent for first-use helper setup. The installer
authorizes your desktop user once; subsequent GUI connections need no password.
DNS supports systemd-resolved, resolvconf, or a backed-up resolv.conf file.
The UI builds on macOS; VPN connections currently have a Linux backend only.

```sh
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo build --release --locked
sudo bash scripts/install.sh install
wave-vpn-gui
```

The GUI runs as your user. A root-owned, socket-activated helper authorizes the
installing user through kernel peer credentials and launches a worker for a fixed
set of networking actions. The worker validates structured WireGuard profile/options
data received through a pipe. It configures its own WireGuard
interface, policy routes and nftables table, and keeps a root-only recovery record.
It never accepts shell commands or raw profile directives. Network lock stays
active after disconnect, failure or worker crash until explicitly disabled. Passwords are prompted privately or entered in the GUI;
sign-in saves credentials automatically in the system keyring. If secure storage
is unavailable, the UI warns before sign-in that credentials will be stored in
plaintext in a private local file. Sign-out removes the saved account.

Generate a private configuration for another VPN manager:

```sh
wave-vpn profile --login YOUR_USERNAME --server SERVER_NAME --output wave.conf
```

The new mode-0600 output file contains private keys. Keep it private.

Optional live bootstrap test, using a synthetic invalid account:

```sh
cargo test --test protocol live_bootstrap -- --ignored
```

## Focused scope

Keep simple sign-in/sign-out with automatic secure storage, read-only Eddie import,
server search/favorites, connect/reconnect, Network Lock, status/logs and tray.
Settings have two tabs and five preferences. Advanced routing, forwarding, DNS
and transport tuning are outside the desktop UI; boot autoconnect is deferred.
See [scope](docs/PARITY.md).

When enabled, Network Lock stays on after Disconnect, Quit and Recover.
The toggle also works during an active tunnel and remembers your confirmed
choice. To restore normal internet access, turn it off in Settings, or use
`sudo wave-vpn lock-disable`. Enabling it while disconnected needs
no additional service installation. The networking helper and boot-persistent
lock are separate. Ordinary lock survives app exit, but reboot persistence
requires the optional `lock-install` / `lock-enable` CLI service. LAN access and
physical-interface ping bypass are off by default. Loopback, DHCP/IPv6 neighbor
discovery, tunnel transport and pinned AirVPN bootstrap endpoints are permitted.

Eddie import accepts XML and v1/v2 profiles with default/password/keyring
protection. Linux keyring import reads Eddie's existing Secret Service item and
does not modify it. Imported credentials fill the sign-in form; successful
sign-in saves them automatically. Advanced Eddie settings are omitted.
Install `libsecret-tools` and unlock your desktop keyring for saved accounts.

Authenticated backend checks pass on Ubuntu GNOME with Comae and Orion: real
IPv4/IPv6 traffic, AirVPN DNS, switching/reconnect and Network Lock after
disconnect/crash/Recover. Exact test scope and remaining UI/platform limits are
in the [verification record](docs/VERIFICATION.md).

Run the isolated networking checks **inside a Linux test VM**:

```sh
cargo build --bins
sudo python3 tests/linux_network.py target/debug/wave-vpn
sudo env WAVE_TEST_SERVICE=1 python3 tests/linux_network.py target/debug/wave-vpn
```

The runner creates private mount/network/PID namespaces and an /etc overlay.
It does not require AirVPN credentials or internet access.

[Verification record](docs/VERIFICATION.md). Version 0.1.0 is an experimental prerelease. See [release notes](docs/RELEASE-v0.1.0.md) for known failures and qualification limits.

## Connection checks and recovery

The app checks the advertised AirVPN tunnel identity for each configured IP
family and verifies that AirVPN received a fresh challenge through your system
DNS. Results appear in Overview. Checks repeat periodically; failed checks stop
the connection and auto-reconnect uses cancellable backoff. A check-service
outage is shown as unavailable, not as proof of a leak. Network Lock follows
its last explicit setting through recovery and retry.

If sign-in cannot reach AirVPN, **Recover** in Overview is available before you
sign in. It restores only Wave's journaled DNS/tunnel resources, keeping an
enabled lock. Bootstrap uses pinned direct-IP endpoints and does not require
working public DNS. Account rejection is shown separately from network failure.
If Eddie left its own DNS/firewall state behind, recover that in Eddie first;
Wave does not reset other applications' networking.

**Logs → Support report** previews safe diagnostics and lets you copy or save
exactly that report. It includes check/protection results and tool availability,
with fixed failure categories. Accounts, keys, addresses and raw logs are omitted;
there is no automatic upload. `wave-vpn diagnostics` offers the CLI metadata
snapshot; `wave-vpn service-client network-status` queries actual firewall and
recovery state without changing networking or requesting new authorization.

[Release gates and signature verification](docs/RELEASE-GATES.md) distinguish
implemented features from the remaining public-release requirements.

## Published history privacy

The repository history and release packages were sanitized on 2026-11-03.
Lab records use example identities and addresses; private management details
are omitted. Release archives and signed checksums were repackaged while keeping
the APK and executable bytes unchanged. Existing clones may retain old history.
