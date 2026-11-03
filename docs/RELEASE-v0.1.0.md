# v0.1.0 — experimental prerelease

Unofficial GPL-3.0-or-later Rust/Iced AirVPN client. Initial packaging targets
Ubuntu 24.04 x86_64 with systemd, kernel WireGuard, nftables and GNOME. Other
architectures/distributions have partial lab evidence, not this binary's full
qualification. macOS builds support UI development, not VPN connections.

Includes sign-in, server search/favorites, connect/reconnect, Network Lock,
Secret Service storage, read-only Eddie import, a privileged socket helper,
provider IPv4/IPv6 and DNS checks, recovery, tray and redacted support reports.

**Experimental:** final-release connection/cancel/switch/retry/stop, tray,
sign-out and PAM workflows remain incompletely qualified. Physical suspend,
the complete installation/upgrade/conflict matrix, broader platform coverage and
security-contact delivery verification remain open. See VERIFICATION.md and
RELEASE-GATES.md; historical debug/installed-release results do not qualify the
new v0.1.0 binary by themselves. Do not treat passing unit tests as proof of
complete VPN protection or production readiness.

Downloads contain Linux binaries, installer/uninstaller, dependency notices,
tagged source, build provenance, checksums and an OpenPGP manifest signature.
Verify the manifest with the independently trusted fingerprint in SECURITY.md.
Install the documented runtime dependencies before running the packaged installer
as administrator. Network Lock intentionally persists after Disconnect/Quit when
enabled; turn it off in Settings or use sudo wave-vpn lock-disable to restore
ordinary Internet access. Optional reboot persistence is a separate opt-in.

No account data, private profiles or signing secrets are distributed. No
production support or security response-time commitment is made.
