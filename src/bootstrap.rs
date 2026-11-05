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
/// Rejects addresses that are not plausible public bootstrap servers.
pub fn public_unicast(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            let o = a.octets();
            !(a.is_unspecified()
                || a.is_loopback()
                || a.is_private()
                || a.is_link_local()
                || a.is_multicast()
                || a.is_broadcast()
                || o[0] == 0
                || o[0] >= 240
                || (o[0] == 100 && (o[1] & 0xc0) == 64))
        }
        IpAddr::V6(a) => {
            let first = a.segments()[0];
            !(a.is_unspecified()
                || a.is_loopback()
                || a.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || a.to_ipv4_mapped().is_some()
                || a.to_ipv4().is_some_and(|_| a.segments()[..6] == [0; 6]))
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
    let mut out = Resolved::new();
    for url in &bootstrap.urls {
        let Ok(parsed) = endpoint(url) else { continue };
        let Some(Err(name)) = host(&parsed) else {
            continue;
        };
        if out.contains_key(&name) || !valid_hostname(&name) {
            continue;
        }
        let mut addresses: Vec<IpAddr> = lookup(&name)
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
        if !addresses.is_empty() {
            out.insert(name, addresses);
        }
    }
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
    /// IP/port pairs Network Lock must allow for this configuration: literal
    /// addresses as listed, hostnames only through `resolved`. Invalid or
    /// unsafe entries are skipped (fail closed); at most `MAX_EXEMPTIONS`.
    pub fn exemptions(&self, resolved: &Resolved) -> Vec<Exemption> {
        let mut out: Vec<Exemption> = Vec::new();
        for url in &self.urls {
            let Ok(parsed) = endpoint(url) else { continue };
            let Some(port) = parsed.port_or_known_default() else {
                continue;
            };
            let addresses = match host(&parsed) {
                Some(Ok(ip)) => vec![ip],
                Some(Err(name)) => resolved.get(&name).cloned().unwrap_or_default(),
                None => vec![],
            };
            for ip in addresses {
                let exemption = Exemption { ip, port };
                if exemption.validate().is_ok() && !out.contains(&exemption) {
                    out.push(exemption);
                }
            }
        }
        out.truncate(MAX_EXEMPTIONS);
        out
    }
}

/// Socket addresses a hostname endpoint is pinned to, if it was resolved.
pub fn pinned(url: &reqwest::Url, resolved: &Resolved) -> Option<Vec<SocketAddr>> {
    let Some(Err(name)) = host(url) else {
        return None;
    };
    let port = url.port_or_known_default()?;
    let addresses = resolved.get(&name)?;
    Some(
        addresses
            .iter()
            .map(|ip| SocketAddr::new(*ip, port))
            .collect(),
    )
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
}

/// The stored latest bootstrap with its manifest time and resolved hostnames.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Latest {
    pub bootstrap: Option<Bootstrap>,
    pub issued: u64,
    pub resolved: Resolved,
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
    /// Validate, then replace atomically unless a newer manifest's bootstrap
    /// is stored. Unchanged data is not rewritten.
    pub fn save(&self, bootstrap: &Bootstrap) -> Result<()> {
        self.save_fenced(bootstrap, 0, &Resolved::new(), &|| true)
            .map(|_| ())
    }
    /// `current` is checked while holding this process's persistence lock and
    /// an exclusive lock on the store, immediately before writing; when it is
    /// false the write is refused with `Cancelled`. A bootstrap from a manifest
    /// older than the stored one is never written (`Saved::Stale`), so an old
    /// session or a concurrent process cannot roll the store back.
    pub fn save_fenced(
        &self,
        bootstrap: &Bootstrap,
        issued: u64,
        resolved: &Resolved,
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
        if let Some(stored) = self.read() {
            let stored_issued = effective(stored.issued);
            if stored_issued > issued {
                return Ok(Saved::Stale);
            }
            if stored_issued == issued
                && stored.bootstrap == *bootstrap
                && stored.resolved == *resolved
            {
                return Ok(Saved::Unchanged);
            }
        }
        let mut temp = tempfile::NamedTempFile::new_in(directory)?;
        temp.write_all(&serde_json::to_vec_pretty(&Stored {
            version: STORE_VERSION,
            issued,
            bootstrap: bootstrap.clone(),
            resolved: resolved.clone(),
        })?)?;
        temp.as_file().sync_all()?;
        temp.persist(&self.path)
            .context("Unable to save bootstrap configuration")?;
        Ok(Saved::Written)
    }
}

/// An advisory lock shared by every process writing the same store.
struct FileLock(#[allow(dead_code)] std::fs::File);
impl FileLock {
    fn exclusive(path: &Path) -> Result<Self> {
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let file = options
            .open(path)
            .context("Unable to lock bootstrap configuration")?;
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
                    "Unable to lock bootstrap configuration: {error}"
                );
            }
        }
        Ok(Self(file))
    }
}
