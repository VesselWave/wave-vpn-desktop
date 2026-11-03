# Security status

Wave VPN 0.1.0 is an experimental prerelease. No version currently has
a production support or security maintenance promise. Do not interpret a passing
unit test or a macOS UI build as verified VPN protection. Release gates and tested
environments are tracked in docs/PARITY.md and docs/VERIFICATION.md.

Report vulnerabilities privately to **vesselwave@protonmail.com**. Include the
app version, operating system and reproduction steps. Do not post passwords,
private profiles, WireGuard keys, account identifiers or unredacted logs in public
issues. This is the maintainer's monitored address. No response-time commitment is
currently offered.

Release downloads use the OpenPGP key **Wave VPN Releases
<vesselwave@protonmail.com>**, fingerprint:

```text
4140 18C0 BBB8 C0A7 0A80 BF57 0AA3 2CCB 2C0B 3951
```

The [public key](docs/keys/wave-release-signing.asc) expires on 2028-10-21.
See [release verification](docs/RELEASE-GATES.md). The release-signing key is for
verifying downloads, not encrypting vulnerability reports. Private key material
and passphrases are not part of this repository.

The GUI is unprivileged. A root-owned systemd socket helper accepts only the
authorized desktop UID (or root), checked with SO_PEERCRED. It accepts only
transport-worker, recover, lock-enable, lock-disable and read-only network-status, with no command paths or
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

Provider verification pins HTTPS checks to advertised AirVPN exit addresses and
binds each request to its corresponding tunnel address. Expected internal VPN
identity and a random system-DNS receipt must match before provider checks are
reported verified. These checks repeat periodically, but do not establish an
exhaustive proof against all leak timing/protocols. Missing provider metadata or
check-service failures are not silently treated as verified. Support reports use
an allowlisted schema with fixed failure categories; raw logs are never exported.

Actual protection queries read the owned nftables table; stale/missing cached
status, failed queries and ownership conflicts cannot establish confirmed
protection. Setup/configuration/recovery failures stop automatic retry; monitored
network failures eventually wait two minutes between attempts. A new worker
repairs only Wave's recovery journal while holding the global networking lock.
Other VPN firewall rules and resolver backups are never used as Wave state.
