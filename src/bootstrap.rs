// SPDX-License-Identifier: GPL-3.0-or-later
//! Bootstrap endpoints and RSA key. Eddie reads them from the latest manifest
//! and keeps the provider definition only as a default; see docs/PROVENANCE.md.
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use rsa::{traits::PublicKeyParts, BigUint, RsaPublicKey};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::Duration,
};

/// More addresses than this in a manifest is malformed, not a longer failover.
pub const MAX_URLS: usize = 16;
/// Network Lock exempts at most this many latest-bootstrap IP/port pairs.
pub const MAX_EXEMPTIONS: usize = 16;
/// Addresses kept per bootstrap hostname.
pub const MAX_ADDRESSES_PER_HOST: usize = 4;
const MAX_URL_LENGTH: usize = 256;
const MAX_STORED: u64 = 64 * 1024;
const STORE_VERSION: u32 = 2;
/// Hostname lookups for bootstrap endpoints give up after this long.
pub const RESOLVE_TIMEOUT: Duration = Duration::from_secs(3);
/// No new hostname lookup starts after this much time adopting one manifest,
/// so adoption waits at most this plus one `RESOLVE_TIMEOUT`.
pub const RESOLVE_BUDGET: Duration = Duration::from_secs(6);
/// A stored manifest time further ahead of the clock than this is ignored, so
/// a wrong clock cannot freeze bootstrap updates forever.
const MAX_CLOCK_SKEW: u64 = 7 * 24 * 3600;

/// One encrypted-bootstrap destination Network Lock may reach outside the
/// tunnel: a literal address and TCP port. Never a hostname, range or service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exemption {
    pub ip: IpAddr,
    pub port: u16,
}
impl Exemption {
    /// Public unicast addresses only, and never a DNS port: an exemption must
    /// not open the LAN, loopback, multicast or a resolver outside the tunnel.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.port != 0 && !matches!(self.port, 53 | 853),
            "Unsafe bootstrap exemption port"
        );
        ensure!(
            public_unicast(self.ip),
            "Unsafe bootstrap exemption address"
        );
        Ok(())
    }
}
/// Globally routable public unicast only, conservatively: IPv4 outside every
/// special-purpose block (RFC 6890 registry: private, shared, loopback,
/// link-local, protocol assignments, documentation, benchmarking, 6to4 relay,
/// multicast, reserved, broadcast); IPv6 only inside 2000::/3 and outside
/// 2001::/23 (Teredo, ORCHID, benchmarking), 2001:db8::/32 and 3fff::/20
/// (documentation) and 2002::/16 (6to4). Transition and translation forms
/// (IPv4-mapped/compatible, NAT64 64:ff9b::/96 and /48, 6to4, Teredo) are
/// refused rather than judged by the IPv4 address they carry; site-local
/// fec0::/10, discard 100::/64, ULA, link-local and multicast fall outside 2000::/3.
pub fn public_unicast(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            let n = u32::from(a);
            const BLOCKED: [(u32, u32); 15] = [
                (0x0000_0000, 8),
                (0x0a00_0000, 8),
                (0x6440_0000, 10),
                (0x7f00_0000, 8),
                (0xa9fe_0000, 16),
                (0xac10_0000, 12),
                (0xc000_0000, 24),
                (0xc000_0200, 24),
                (0xc058_6300, 24),
                (0xc0a8_0000, 16),
                (0xc612_0000, 15),
                (0xc633_6400, 24),
                (0xcb00_7100, 24),
                (0xe000_0000, 4),
                (0xf000_0000, 4),
            ];
            !BLOCKED
                .iter()
                .any(|(net, len)| n >> (32 - len) == net >> (32 - len))
        }
        IpAddr::V6(a) => {
            let n = u128::from(a);
            let within = |net: u128, len: u32| n >> (128 - len) == net >> (128 - len);
            within(0x2000 << 112, 3)
                && ![
                    (0x2001u128 << 112, 23),
                    (0x2001_0db8u128 << 96, 32),
                    (0x2002u128 << 112, 16),
                    (0x3fffu128 << 112, 20),
                ]
                .iter()
                .any(|(net, len)| within(*net, *len))
        }
    }
}
/// Bounded list of valid exemptions, as accepted by the privileged helper.
pub fn validate_exemptions(list: &[Exemption]) -> Result<()> {
    ensure!(
        list.len() <= MAX_EXEMPTIONS,
        "Too many bootstrap exemptions"
    );
    for exemption in list {
        exemption.validate()?;
    }
    Ok(())
}

/// Addresses for bootstrap hostnames, resolved by the unprivileged desktop
/// while the network is usable. The helper never resolves names; it receives
/// only the validated literal addresses, and the client pins each hostname to
/// these addresses so it needs no DNS outside the tunnel under Network Lock.
pub type Resolved = BTreeMap<String, Vec<IpAddr>>;

fn valid_hostname(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
fn validate_resolved(resolved: &Resolved) -> Result<()> {
    ensure!(
        resolved.len() <= MAX_URLS,
        "Too many resolved bootstrap hosts"
    );
    for (host, addresses) in resolved {
        ensure!(valid_hostname(host), "Invalid bootstrap hostname");
        ensure!(
            !addresses.is_empty() && addresses.len() <= MAX_ADDRESSES_PER_HOST,
            "Invalid resolved bootstrap addresses"
        );
        ensure!(
            addresses.iter().all(|ip| public_unicast(*ip)),
            "Unsafe resolved bootstrap address"
        );
    }
    Ok(())
}
/// The host of an endpoint, split into a literal address or a hostname.
fn host(url: &reqwest::Url) -> Option<Result<IpAddr, String>> {
    let text = url.host_str()?;
    if let Some(v6) = text.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
        return v6.parse().ok().map(Ok);
    }
    Some(
        text.parse::<IpAddr>()
            .map_err(|_| text.to_ascii_lowercase()),
    )
}

/// Resolve the hostnames of `bootstrap` with `lookup`, keeping at most four
/// public addresses per host. A failed lookup keeps the host's previous
/// addresses; hosts no longer listed are dropped. Literal addresses need none.
pub fn resolve_hosts(
    bootstrap: &Bootstrap,
    previous: &Resolved,
    lookup: &dyn Fn(&str) -> Option<Vec<IpAddr>>,
) -> Resolved {
    resolve_hosts_bounded(bootstrap, previous, lookup, &|| true)
}
/// As `resolve_hosts`, but no new lookup starts once `keep_going` is false
/// (cancelled or out of the total time budget); remaining hosts keep their
/// previous addresses. The result is trimmed to the addresses that fit the
/// shared exemption budget, so every pinned address is also exempted.
pub fn resolve_hosts_bounded(
    bootstrap: &Bootstrap,
    previous: &Resolved,
    lookup: &dyn Fn(&str) -> Option<Vec<IpAddr>>,
    keep_going: &dyn Fn() -> bool,
) -> Resolved {
    let mut out = Resolved::new();
    for url in &bootstrap.urls {
        let Ok(parsed) = endpoint(url) else { continue };
        let Some(Err(name)) = host(&parsed) else {
            continue;
        };
        if out.contains_key(&name) || !valid_hostname(&name) {
            continue;
        }
        let looked_up = if keep_going() { lookup(&name) } else { None };
        let mut addresses: Vec<IpAddr> = looked_up
            .unwrap_or_default()
            .into_iter()
            .filter(|ip| public_unicast(*ip))
            .collect();
        addresses.sort();
        addresses.dedup();
        addresses.truncate(MAX_ADDRESSES_PER_HOST);
        if addresses.is_empty() {
            if let Some(kept) = previous.get(&name) {
                addresses = kept.clone();
            }
        }
        addresses.retain(|ip| public_unicast(*ip));
        if !addresses.is_empty() {
            out.insert(name, addresses);
        }
    }
    let allowed = bootstrap.exemptions(&out);
    out.retain(|_, addresses| {
        addresses.retain(|ip| allowed.iter().any(|e| e.ip == *ip));
        !addresses.is_empty()
    });
    out
}
/// The system resolver, bounded by `RESOLVE_TIMEOUT` (a lookup that never
/// returns is abandoned on its own thread).
pub fn system_lookup(name: &str) -> Option<Vec<IpAddr>> {
    use std::net::ToSocketAddrs;
    let (tx, rx) = std::sync::mpsc::channel();
    let name = name.to_owned();
    std::thread::spawn(move || {
        let result = (name.as_str(), 0)
            .to_socket_addrs()
            .map(|a| a.map(|s| s.ip()).collect::<Vec<_>>());
        let _ = tx.send(result);
    });
    rx.recv_timeout(RESOLVE_TIMEOUT).ok()?.ok()
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bootstrap {
    pub urls: Vec<String>,
    pub modulus: String,
    pub exponent: String,
}
impl Bootstrap {
    /// The pinned Eddie provider definition compiled into the binary.
    pub fn eddie() -> Self {
        let definition: serde_json::Value =
            serde_json::from_str(include_str!("../protocol-data/AirVPN.json"))
                .expect("pinned Eddie provider definition");
        let m = &definition["manifest"];
        Self {
            urls: m["urls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["address"].as_str().unwrap().to_owned())
                .collect(),
            modulus: m["auth_rsa_modulus"].as_str().unwrap().to_owned(),
            exponent: m["auth_rsa_exponent"].as_str().unwrap().to_owned(),
        }
    }
    pub fn public_key(&self) -> Result<RsaPublicKey> {
        let key = RsaPublicKey::new(
            BigUint::from_bytes_be(&STANDARD.decode(&self.modulus)?),
            BigUint::from_bytes_be(&STANDARD.decode(&self.exponent)?),
        )?;
        Ok(key)
    }
    /// A usable bootstrap has 1..=16 plain HTTP(S) endpoints and an RSA key of
    /// at least 2048 bits. Anything else keeps the previous configuration.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.urls.is_empty() && self.urls.len() <= MAX_URLS,
            "Bootstrap must list 1 to {MAX_URLS} addresses"
        );
        for url in &self.urls {
            endpoint(url)?;
        }
        ensure!(
            self.public_key()?.size() * 8 >= 2048,
            "Bootstrap RSA key is too short"
        );
        Ok(())
    }
}

impl Bootstrap {
    /// The one bounded IP/port selection shared by the helper's Network Lock
    /// policy and the client's hostname pinning: literal addresses in listed
    /// order, then resolved hostname addresses round-robin (each host's first
    /// address, then each host's second, ...), so every host keeps addresses
    /// within `MAX_EXEMPTIONS`. Invalid or unsafe entries are skipped.
    pub fn exemptions(&self, resolved: &Resolved) -> Vec<Exemption> {
        let mut out: Vec<Exemption> = Vec::new();
        let push = |out: &mut Vec<Exemption>, exemption: Exemption| {
            if out.len() < MAX_EXEMPTIONS
                && exemption.validate().is_ok()
                && !out.contains(&exemption)
            {
                out.push(exemption);
            }
        };
        let mut hosts: Vec<(&[IpAddr], u16)> = Vec::new();
        for url in &self.urls {
            let Ok(parsed) = endpoint(url) else { continue };
            let Some(port) = parsed.port_or_known_default() else {
                continue;
            };
            match host(&parsed) {
                Some(Ok(ip)) => push(&mut out, Exemption { ip, port }),
                Some(Err(name)) => {
                    if let Some(addresses) = resolved.get(&name) {
                        hosts.push((addresses, port));
                    }
                }
                None => (),
            }
        }
        for round in 0..MAX_ADDRESSES_PER_HOST {
            for (addresses, port) in &hosts {
                if let Some(ip) = addresses.get(round) {
                    push(
                        &mut out,
                        Exemption {
                            ip: *ip,
                            port: *port,
                        },
                    );
                }
            }
        }
        out
    }
    /// Socket addresses a hostname endpoint is pinned to: only resolved
    /// addresses inside this configuration's exemption selection. None for a
    /// literal address or a hostname without such an address.
    pub fn pinned(&self, url: &reqwest::Url, resolved: &Resolved) -> Option<Vec<SocketAddr>> {
        let Some(Err(name)) = host(url) else {
            return None;
        };
        let port = url.port_or_known_default()?;
        let allowed = self.exemptions(resolved);
        let pins: Vec<SocketAddr> = resolved
            .get(&name)?
            .iter()
            .filter(|ip| allowed.contains(&Exemption { ip: **ip, port }))
            .map(|ip| SocketAddr::new(*ip, port))
            .collect();
        (!pins.is_empty()).then_some(pins)
    }
}

/// Whether an endpoint names a host (needs DNS or a pin) rather than an address.
pub fn is_hostname(url: &reqwest::Url) -> bool {
    matches!(host(url), Some(Err(_)))
}

/// Parse one bootstrap address; credentials, queries and fragments are refused.
pub fn endpoint(url: &str) -> Result<reqwest::Url> {
    ensure!(
        url.len() <= MAX_URL_LENGTH && !url.chars().any(char::is_control),
        "Invalid bootstrap URL"
    );
    let parsed = reqwest::Url::parse(url).context("Invalid bootstrap URL")?;
    ensure!(
        matches!(parsed.scheme(), "http" | "https")
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.host_str().is_some_and(|h| !h.is_empty())
            && parsed.query().is_none()
            && parsed.fragment().is_none(),
        "Invalid bootstrap URL"
    );
    Ok(parsed)
}

/// Configurations to try, latest manifest first, pinned definition last.
pub fn candidates(latest: Option<Bootstrap>) -> Vec<Bootstrap> {
    let pinned = Bootstrap::eddie();
    match latest {
        Some(latest) if latest != pinned && latest.validate().is_ok() => vec![latest, pinned],
        _ => vec![pinned],
    }
}

/// Every distinct (configuration, address) pair once, in failover order.
/// Within a configuration IPv6 literals go last, as before; an address already
/// tried with the same key is not repeated.
pub fn attempts(configurations: &[Bootstrap]) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for (index, configuration) in configurations.iter().enumerate() {
        let mut urls = configuration.urls.clone();
        urls.sort_by_key(|u| u.contains('['));
        for url in urls {
            let repeated = out.iter().any(|(i, u)| {
                *u == url
                    && configurations[*i].modulus == configuration.modulus
                    && configurations[*i].exponent == configuration.exponent
            });
            if !repeated {
                out.push((index, url));
            }
        }
    }
    out
}

/// Pause before failover attempt `attempt` (0 is the first, immediate attempt):
/// 250 ms doubling to at most 2 s, so an outage is not hammered in a burst.
pub fn failover_delay(attempt: usize) -> Duration {
    if attempt == 0 {
        Duration::ZERO
    } else {
        Duration::from_millis(250u64 << (attempt - 1).min(3))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    /// The manifest's own `time` (Unix seconds); 0 when it had none. A write
    /// from an older manifest never replaces a newer one.
    #[serde(default)]
    issued: u64,
    bootstrap: Bootstrap,
    #[serde(default)]
    resolved: Resolved,
    /// Incremented by every write. A session captures it before its request
    /// and may replace an equal-time configuration only if it is unchanged.
    #[serde(default)]
    revision: u64,
}

/// The stored latest bootstrap with its manifest time and resolved hostnames.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Latest {
    pub bootstrap: Option<Bootstrap>,
    pub issued: u64,
    pub resolved: Resolved,
    /// Store revision; pass it to `save_fenced` as the session's base.
    pub revision: u64,
}

/// What a fenced save did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Saved {
    Written,
    Unchanged,
    /// A newer manifest's bootstrap is already stored; nothing was written.
    Stale,
}

/// Serializes bootstrap writes in this process with cancellation: holding it,
/// a writer checks that its session is still current, and `client::cancel`
/// takes it before cancelling. Once cancel returns, no write from the
/// cancelled session can still land.
static PERSIST: Mutex<()> = Mutex::new(());
pub(crate) fn persistence_lock() -> MutexGuard<'static, ()> {
    PERSIST.lock().unwrap_or_else(|e| e.into_inner())
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
/// Manifest times from the far future are treated as unknown.
fn effective(issued: u64) -> u64 {
    if issued > now_unix().saturating_add(MAX_CLOCK_SKEW) {
        0
    } else {
        issued
    }
}

/// The latest validated manifest bootstrap, kept in the user's config directory.
/// It holds only public endpoints, a public key, the manifest time and the
/// addresses of bootstrap hostnames; no account data.
#[derive(Clone, Debug)]
pub struct BootstrapStore {
    path: PathBuf,
}
impl BootstrapStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn in_directory(directory: &Path) -> Self {
        Self::new(directory.join("bootstrap.json"))
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    fn read(&self) -> Option<Stored> {
        let meta = std::fs::symlink_metadata(&self.path).ok()?;
        if !meta.is_file() || meta.len() > MAX_STORED {
            return None;
        }
        let mut bytes = Vec::new();
        std::fs::File::open(&self.path)
            .ok()?
            .take(MAX_STORED + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        let stored: Stored = serde_json::from_slice(&bytes).ok()?;
        (matches!(stored.version, 1 | STORE_VERSION)
            && stored.bootstrap.validate().is_ok()
            && validate_resolved(&stored.resolved).is_ok())
        .then_some(stored)
    }
    /// Missing, oversized, symlinked, malformed or invalid files read as none,
    /// so the embedded definition still works on first run or after damage.
    pub fn load(&self) -> Option<Bootstrap> {
        self.read().map(|s| s.bootstrap)
    }
    pub fn latest(&self) -> Latest {
        self.read()
            .map(|s| Latest {
                bootstrap: Some(s.bootstrap),
                issued: effective(s.issued),
                resolved: s.resolved,
                revision: s.revision,
            })
            .unwrap_or_default()
    }
    /// What Network Lock must allow for the stored latest bootstrap (the
    /// embedded definition is always allowed by the policy itself).
    pub fn exemptions(&self) -> Vec<Exemption> {
        let latest = self.latest();
        latest
            .bootstrap
            .map(|b| b.exemptions(&latest.resolved))
            .unwrap_or_default()
    }
    /// The current revision, to capture before a request (0 when none).
    pub fn revision(&self) -> u64 {
        self.read().map_or(0, |s| s.revision)
    }
    /// Validate, then replace atomically unless a newer manifest's bootstrap
    /// is stored. Unchanged data is not rewritten. Not session-fenced: for
    /// explicit local writes, never for a network response.
    pub fn save(&self, bootstrap: &Bootstrap) -> Result<()> {
        self.write(bootstrap, 0, &Resolved::new(), None, &|| true)
            .map(|_| ())
    }
    /// `current` is checked while holding this process's persistence lock and
    /// an exclusive lock on the store, immediately before writing; when it is
    /// false the write is refused with `Cancelled`. Ordering, decided under
    /// the file lock (`Saved::Stale` writes nothing):
    /// * an older manifest time than the stored one never replaces it;
    /// * a strictly newer known time always does;
    /// * an equal time, including both unknown (0), replaces it only if the
    ///   store is still at `base`, the revision this session captured before
    ///   its request. A late response from a session that started before
    ///   another write, in this or any process, cannot replace that write,
    ///   while a session that saw the current state may still re-resolve its
    ///   hostnames for the same manifest.
    pub fn save_fenced(
        &self,
        bootstrap: &Bootstrap,
        issued: u64,
        resolved: &Resolved,
        base: u64,
        current: &dyn Fn() -> bool,
    ) -> Result<Saved> {
        self.write(bootstrap, issued, resolved, Some(base), current)
    }
    fn write(
        &self,
        bootstrap: &Bootstrap,
        issued: u64,
        resolved: &Resolved,
        base: Option<u64>,
        current: &dyn Fn() -> bool,
    ) -> Result<Saved> {
        bootstrap.validate()?;
        validate_resolved(resolved)?;
        let issued = effective(issued);
        let directory = self
            .path
            .parent()
            .context("Bootstrap store has no directory")?;
        let _process = persistence_lock();
        std::fs::create_dir_all(directory)?;
        ensure!(
            std::fs::symlink_metadata(directory)?.is_dir(),
            "Bootstrap directory must not be a symlink"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
        }
        let _file = FileLock::exclusive(&self.path.with_extension("lock"))?;
        if !current() {
            return Err(crate::client::Cancelled.into());
        }
        let stored = self.read();
        let mut revision = base.unwrap_or(0);
        if let Some(stored) = &stored {
            let stored_issued = effective(stored.issued);
            if stored_issued > issued {
                return Ok(Saved::Stale);
            }
            if stored_issued == issued {
                if stored.bootstrap == *bootstrap && stored.resolved == *resolved {
                    return Ok(Saved::Unchanged);
                }
                if base.is_some_and(|base| base != stored.revision) {
                    return Ok(Saved::Stale);
                }
            }
            revision = revision.max(stored.revision);
        }
        let mut temp = tempfile::NamedTempFile::new_in(directory)?;
        temp.write_all(&serde_json::to_vec_pretty(&Stored {
            version: STORE_VERSION,
            issued,
            bootstrap: bootstrap.clone(),
            resolved: resolved.clone(),
            revision: revision.wrapping_add(1),
        })?)?;
        temp.as_file().sync_all()?;
        temp.persist(&self.path)
            .context("Unable to save bootstrap configuration")?;
        Ok(Saved::Written)
    }
}

/// An advisory lock shared by every process writing the same store.
pub(crate) struct FileLock(#[allow(dead_code)] std::fs::File);
impl FileLock {
    pub(crate) fn exclusive(path: &Path) -> Result<Self> {
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let file = options
            .open(path)
            .context("Unable to lock configuration file")?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            loop {
                // Blocks only while another process writes a few kilobytes.
                if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
                    break;
                }
                let error = std::io::Error::last_os_error();
                ensure!(
                    error.kind() == std::io::ErrorKind::Interrupted,
                    "Unable to lock configuration file: {error}"
                );
            }
        }
        Ok(Self(file))
    }
}
