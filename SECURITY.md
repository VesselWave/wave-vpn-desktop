# Security status

Wave VPN 0.2.0 is an unpublished development version. No version currently has
a production support or security maintenance promise. Do not interpret a passing
unit test or a macOS UI build as verified VPN protection. Release gates and tested
environments are tracked in docs/PARITY.md and docs/VERIFICATION.md.

Report vulnerabilities privately using the repository's **Security → Report a
vulnerability** action when enabled. Do not post passwords, private profiles,
WireGuard keys, account identifiers or unredacted logs in issues. If private
reporting is unavailable, the maintainer must configure a private contact before
release; no fallback security email is asserted here.

The GUI is unprivileged. A root-owned systemd socket helper accepts only the
authorized desktop UID (or root), checked with SO_PEERCRED. It accepts only
transport-worker, recover, lock-enable and lock-disable, with no command paths or
caller environment. Installing or replacing the helper requires administrator
authorization. Helper binaries and their parent paths must be root-owned and
not writable by other users; the authorization file is root-only. Networking
workers retain validated structured input, fixed tool locations, a root-only
recovery journal and a separate nftables table. User preferences contain no account password or tunnel
keys. Credentials are saved automatically to Linux Secret Service when available. If
no usable keyring service exists, the UI warns before sign-in and credentials
are saved in a mode-0600 `account.json` within the mode-0700 user configuration
directory. This fallback is plaintext, not encryption. Locked/declined keyring
operations do not silently switch to plaintext. Sign-out removes the saved item
and fallback file; a deletion failure keeps the account signed in so it can be
retried. Memory wiping reduces exposure but
is not a guarantee against memory copies, swap or a compromised desktop session.

AirVPN bootstrap follows pinned Eddie RSA/AES-CBC semantics, including direct-IP
HTTP bootstrap. That legacy wire format is not AEAD; see PROVENANCE.md. Eddie
profile import checks HMAC before decrypting, accepts only mapped settings and
leaves imported credentials in the sign-in form for review. Linux keyring profiles use read-only Secret Service lookup of the Eddie profile key.

Network lock intentionally permits pinned bootstrap endpoints, DHCP, IPv6
neighbor discovery and user-configured exceptions. LAN and ping exceptions default off; LAN access can be explicitly enabled. TCP/UDP port 53/853
DNS fallback outside the tunnel is blocked under lock even with LAN exceptions.
DNS-over-HTTPS cannot be distinguished from other HTTPS to explicitly allowed
addresses. Existing firewall rules may restrict Wave; Wave does not flush them.

A lost controller pipe, worker termination or handshake failure retains session
protection. Disconnect and `sudo wave-vpn recover` restore tunnel resources while
retaining an enabled Network Lock. Explicitly disabled protection stays off. Only explicitly turning off the **Network Lock** toggle or
`sudo wave-vpn lock-disable` restores ordinary internet access. Reboot behavior and service ordering remain release
gates. Do not manually delete recovery files while troubleshooting.
