# Security

**Wave VPN is in early release.** Anyone is welcome to use it, but it hasn't been fully tested on every device and system yet. There's no security maintenance promise yet; see [verification](docs/VERIFICATION.md) for what has been tested.

## Reporting a vulnerability

Email **vesselwave@protonmail.com** (monitored by the maintainer). Include the app version, your OS and steps to reproduce. Don't put passwords, private profiles, WireGuard keys, account names or unredacted logs in public issues. There's no response-time commitment yet.

## Verifying releases

Downloads are signed with **Wave VPN Releases &lt;vesselwave@protonmail.com&gt;**:

```text
4140 18C0 BBB8 C0A7 0A80 BF57 0AA3 2CCB 2C0B 3951
```

The same fingerprint is on [keys.openpgp.org](https://keys.openpgp.org/search?q=414018C0BBB8C0A70A80BF570AA32CCB2C0B3951) and [vpn.vwave.me](https://vpn.vwave.me/#verify). The [public key](docs/keys/wave-release-signing.asc) expires on 2028-10-21. It verifies downloads; it isn't for encrypting reports. Private keys and passphrases are never in this repository. See [release gates](docs/RELEASE-GATES.md).

## Privilege separation

- The GUI runs as your user.
- A root-owned systemd socket helper accepts connections only from the authorized desktop UID or root, checked with `SO_PEERCRED`. It accepts only five actions (`transport-worker`, `recover`, `lock-enable`, `lock-disable` and read-only `network-status`), with no command paths and none of the caller's environment.
- Installing or replacing the helper needs administrator authorization. Helper binaries and their parent directories must be root-owned and not writable by others; the authorization file is root-only.
- Networking workers accept only validated structured input, use fixed tool paths, keep a root-only recovery journal and use their own nftables table.

## Credentials

- Saved automatically in the Linux Secret Service when available.
- With no usable keyring, the app warns before sign-in, then saves them in `account.json` (mode 0600) inside the config directory (mode 0700). **This fallback is plain text, not encryption.** A locked or declined keyring never silently falls back to plain text.
- Preferences never contain your password or tunnel keys.
- Sign-out removes the keyring item and the fallback file. If deletion fails, you stay signed in so you can retry.
- Memory is wiped after use, which reduces exposure but can't rule out copies in memory, swap or a compromised desktop session.

## AirVPN protocol and Eddie import

- Sign-in follows Eddie's pinned RSA/AES-CBC scheme, including direct-IP HTTP bootstrap. That legacy wire format isn't authenticated encryption (AEAD); see [provenance](docs/PROVENANCE.md).
- Eddie import checks the HMAC before decrypting, accepts only mapped settings, and leaves imported credentials in the sign-in form for you to review. Keyring-protected Eddie profiles are read with a read-only Secret Service lookup.

## Network Lock

- Allows only the pinned sign-in endpoints, DHCP, IPv6 neighbour discovery and exceptions you turn on. LAN and ping exceptions are off by default.
- DNS on TCP/UDP ports 53 and 853 outside the tunnel is blocked even with LAN access on. DNS-over-HTTPS to an explicitly allowed address can't be told apart from other HTTPS.
- Existing firewall rules may restrict Wave VPN; it never flushes them.

## Failures and recovery

- A lost controller pipe, a worker crash or a failed handshake keeps protection in place.
- Disconnect and `sudo wave-vpn recover` remove tunnel resources and keep an enabled Network Lock. A lock you turned off stays off.
- Only turning off **Network Lock** or `sudo wave-vpn lock-disable` restores ordinary internet access. Don't delete recovery files by hand.
- Protection status is read from Wave's own nftables table. A stale or missing cache, a failed query or an ownership conflict never counts as confirmed protection.
- Setup, configuration and recovery failures stop automatic retry. Monitored network failures back off to two minutes between attempts.
- A new worker repairs only Wave's own recovery journal, while holding the global networking lock. Other VPNs' firewall rules and resolver backups are never treated as Wave's.
- Reboot behaviour and service ordering are still release gates.

## Connection checks and support reports

- Checks go over HTTPS, pinned to AirVPN's advertised exit addresses and bound to the matching tunnel address. Both the expected VPN identity and a random DNS receipt must match before a check counts as verified.
- Checks repeat periodically, but they can't prove the absence of every leak timing or protocol. Missing provider data or a check-service failure is never shown as verified.
- Support reports use an allowlisted format with fixed failure categories. Raw logs are never exported.
