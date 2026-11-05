// SPDX-License-Identifier: GPL-3.0-or-later
//! Server latency: few probes in parallel, cached, never while a tunnel is up.
use crate::Server;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

/// Probes in flight at once, instead of one per server in large batches.
pub const PARALLEL: usize = 4;
/// A complete measurement is reused for this long, whatever triggers a refresh.
pub const MIN_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// After an interrupted or failed run (offline, firewalled), wait this long.
pub const RETRY_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// Servers probed per run: one per location, not the whole fleet.
pub const SAMPLE_BUDGET: usize = 32;
/// A result older than this is forgotten (and its location probed again).
pub const MAX_AGE: Duration = Duration::from_secs(24 * 3600);
/// Upper bound for a single probe, whatever the pinger timeout option says.
pub const MAX_TIMEOUT: Duration = Duration::from_secs(5);
/// Ranking cost of a server that did not answer: after every server that did.
pub const UNREACHABLE_COST: u32 = 10_000;
const PORT: u16 = 443;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ping {
    /// Not measured (yet). Contributes nothing on its own.
    Unknown,
    Measured(u32),
    /// Probed and did not answer in time. Never counts as fast.
    Unreachable,
}
impl Ping {
    pub fn cost(self) -> u32 {
        match self {
            Self::Unknown => 0,
            Self::Measured(ms) => ms,
            Self::Unreachable => UNREACHABLE_COST,
        }
    }
    /// Sort key for the ping column: measured, then unknown, then unreachable.
    pub fn order(self) -> u32 {
        match self {
            Self::Measured(ms) => ms.min(u32::MAX - 2),
            Self::Unknown => u32::MAX - 1,
            Self::Unreachable => u32::MAX,
        }
    }
}

pub trait Prober: Sync {
    /// Round-trip estimate, or None when the target did not answer in time.
    fn probe(&self, target: SocketAddr, timeout: Duration) -> Option<Duration>;
}
/// TCP connect to the entry address, closed immediately. The connect is
/// non-blocking and gives up within 50 ms of `cancel`, so no probe is still
/// in flight once a connection has started.
pub struct TcpProber {
    pub cancel: Arc<AtomicBool>,
}
impl Prober for TcpProber {
    #[cfg(unix)]
    fn probe(&self, target: SocketAddr, timeout: Duration) -> Option<Duration> {
        use socket2::{Domain, Protocol, Socket, Type};
        use std::os::fd::AsRawFd;
        if self.cancel.load(Ordering::Relaxed) {
            return None;
        }
        let socket = Socket::new(
            Domain::for_address(target),
            Type::STREAM,
            Some(Protocol::TCP),
        )
        .ok()?;
        socket.set_nonblocking(true).ok()?;
        let start = Instant::now();
        match socket.connect(&target.into()) {
            Ok(()) => return Some(start.elapsed()),
            Err(e)
                if e.raw_os_error() == Some(libc::EINPROGRESS)
                    || e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return None,
        }
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return None;
            }
            let left = timeout.checked_sub(start.elapsed())?;
            let step = left.min(Duration::from_millis(50)).as_millis().max(1);
            let mut poll = libc::pollfd {
                fd: socket.as_raw_fd(),
                events: libc::POLLOUT,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut poll, 1, step as libc::c_int) };
            if ready < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return None;
            }
            if ready == 0 {
                continue;
            }
            let elapsed = start.elapsed();
            return (matches!(socket.take_error(), Ok(None)) && socket.peer_addr().is_ok())
                .then_some(elapsed);
        }
    }
    #[cfg(not(unix))]
    fn probe(&self, target: SocketAddr, timeout: Duration) -> Option<Duration> {
        if self.cancel.load(Ordering::Relaxed) {
            return None;
        }
        let start = Instant::now();
        std::net::TcpStream::connect_timeout(&target, timeout).ok()?;
        Some(start.elapsed())
    }
}

/// One target per server: its first IPv4 entry address, as before.
pub fn targets(servers: &[Server]) -> Vec<(String, SocketAddr)> {
    servers
        .iter()
        .filter_map(|s| {
            let ip = s.entries.iter().copied().find(IpAddr::is_ipv4)?;
            Some((s.name.clone(), SocketAddr::new(ip, PORT)))
        })
        .collect()
}
/// Servers at one location share a data centre, so one probe stands for all.
pub fn location(server: &Server) -> String {
    format!("{}|{}", server.country, server.location.to_lowercase())
}
fn mix(seed: u64, text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    text.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Complete(BTreeMap<String, Ping>),
    /// A connection started or the user signed out; partial results are dropped.
    Cancelled,
    /// Nothing answered: the network (or Network Lock) is down, not every server.
    Offline,
}

pub fn measure(
    targets: &[(String, SocketAddr)],
    prober: &dyn Prober,
    timeout: Duration,
    parallel: usize,
    cancel: &AtomicBool,
) -> Outcome {
    let timeout = timeout.clamp(Duration::from_millis(100), MAX_TIMEOUT);
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::with_capacity(targets.len()));
    let workers = parallel.clamp(1, PARALLEL).min(targets.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some((name, target)) = targets.get(index) else {
                    break;
                };
                let ping = match prober.probe(*target, timeout) {
                    Some(rtt) if rtt <= timeout => {
                        Ping::Measured(u32::try_from(rtt.as_millis()).unwrap_or(u32::MAX).max(1))
                    }
                    _ => Ping::Unreachable,
                };
                results.lock().unwrap().push((name.clone(), ping));
            });
        }
    });
    if cancel.load(Ordering::Relaxed) {
        return Outcome::Cancelled;
    }
    let results: BTreeMap<_, _> = results.into_inner().unwrap().into_iter().collect();
    if !results.is_empty() && !results.values().any(|p| matches!(p, Ping::Measured(_))) {
        return Outcome::Offline;
    }
    Outcome::Complete(results)
}

/// Cached results and the rules for when another run may start.
#[derive(Debug, Default)]
pub struct LatencyCache {
    pings: BTreeMap<String, (Ping, Instant)>,
    /// Server name to `location()`, for estimates of servers not probed.
    locations: BTreeMap<String, String>,
    measured_at: Option<Instant>,
    next: Option<Instant>,
    generation: u64,
    running: bool,
}
impl LatencyCache {
    /// Probe only when idle, outside any tunnel activity and past the interval.
    pub fn due(&self, now: Instant, tunnel_active: bool) -> bool {
        !tunnel_active && !self.running && self.next.is_none_or(|t| now >= t)
    }
    pub fn running(&self) -> bool {
        self.running
    }
    pub fn measured_at(&self) -> Option<Instant> {
        self.measured_at
    }
    /// When the next run may start (None: now).
    pub fn next(&self) -> Option<Instant> {
        self.next
    }
    /// Start a run; the returned generation identifies its result.
    pub fn begin(&mut self, now: Instant) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.running = true;
        self.next = Some(now + RETRY_INTERVAL);
        self.generation
    }
    /// Results of a cancelled run are ignored even if they still arrive.
    pub fn cancel(&mut self) {
        if self.running {
            self.running = false;
            self.generation = self.generation.wrapping_add(1);
        }
    }
    /// Returns whether the result was accepted. A sample updates the servers
    /// it measured and keeps earlier results for the rest until `MAX_AGE`.
    pub fn finish(&mut self, generation: u64, now: Instant, outcome: Outcome) -> bool {
        if generation != self.generation || !self.running {
            return false;
        }
        self.running = false;
        match outcome {
            Outcome::Complete(pings) => {
                for (name, ping) in pings {
                    self.pings.insert(name, (ping, now));
                }
                self.pings
                    .retain(|_, (_, at)| now.saturating_duration_since(*at) < MAX_AGE);
                self.measured_at = Some(now);
                self.next = Some(now + MIN_INTERVAL);
                true
            }
            Outcome::Offline | Outcome::Cancelled => {
                self.next = Some(now + RETRY_INTERVAL);
                false
            }
        }
    }
    pub fn clear(&mut self) {
        let generation = self.generation.wrapping_add(1);
        *self = Self {
            generation,
            ..Self::default()
        };
    }
    /// Sign-out: drop every result but keep the cooldown, so signing in again
    /// does not start another run before it would otherwise be due.
    pub fn forget(&mut self) {
        let next = self.next;
        self.clear();
        self.next = next;
    }
    pub fn insert(&mut self, name: String, ping: Ping) {
        self.pings.insert(name, (ping, Instant::now()));
    }
    /// Remember which servers share a location (call when the list changes).
    pub fn set_locations(&mut self, servers: &[Server]) {
        self.locations = servers
            .iter()
            .map(|s| (s.name.clone(), location(s)))
            .collect();
    }
    /// What the user sees: the server's own result, else the best result
    /// measured at the same location (same data centre), else unknown.
    pub fn get(&self, name: &str) -> Ping {
        if let Some((ping, _)) = self.pings.get(name) {
            return *ping;
        }
        let Some(place) = self.locations.get(name) else {
            return Ping::Unknown;
        };
        self.locations
            .iter()
            .filter(|(other, l)| *l == place && other.as_str() != name)
            .filter_map(|(other, _)| match self.pings.get(other) {
                Some((Ping::Measured(ms), _)) => Some(*ms),
                _ => None,
            })
            .min()
            .map_or(Ping::Unknown, Ping::Measured)
    }
    /// What the ranking uses: a server with no result for itself or its
    /// location is neither favoured nor punished; it ranks at the median.
    pub fn ranking(&self, name: &str) -> Ping {
        match self.get(name) {
            Ping::Unknown => {
                let mut measured: Vec<u32> = self
                    .pings
                    .values()
                    .filter_map(|(p, _)| match p {
                        Ping::Measured(ms) => Some(*ms),
                        _ => None,
                    })
                    .collect();
                if measured.is_empty() {
                    return Ping::Unknown;
                }
                measured.sort_unstable();
                Ping::Measured(measured[measured.len() / 2])
            }
            ping => ping,
        }
    }
    /// At most `budget` targets for the next run: first the `priority`
    /// servers (an explicitly selected server), then one server per location,
    /// unmeasured locations first, then the longest since measured. Ties and
    /// the representative within a location follow `seed`, never the name
    /// order, so every run spreads differently over the fleet.
    pub fn sample(
        &self,
        servers: &[Server],
        budget: usize,
        priority: &[&str],
        seed: u64,
    ) -> Vec<(String, SocketAddr)> {
        let addresses: BTreeMap<String, SocketAddr> = targets(servers).into_iter().collect();
        let mut out: Vec<(String, SocketAddr)> = Vec::new();
        let mut covered = std::collections::BTreeSet::new();
        for name in priority {
            if out.len() >= budget {
                break;
            }
            if let (Some(address), Some(server)) = (
                addresses.get(*name),
                servers.iter().find(|s| s.name == *name),
            ) {
                if !out.iter().any(|(n, _)| n == name) {
                    out.push((name.to_string(), *address));
                    covered.insert(location(server));
                }
            }
        }
        let mut groups: BTreeMap<String, Vec<&Server>> = BTreeMap::new();
        for server in servers.iter().filter(|s| addresses.contains_key(&s.name)) {
            groups.entry(location(server)).or_default().push(server);
        }
        let mut places: Vec<(Option<Instant>, u64, &String, &Vec<&Server>)> = groups
            .iter()
            .filter(|(place, _)| !covered.contains(*place))
            .map(|(place, members)| {
                let freshest = members
                    .iter()
                    .filter_map(|s| self.pings.get(&s.name).map(|(_, at)| *at))
                    .max();
                (freshest, mix(seed, place), place, members)
            })
            .collect();
        places.sort_by_key(|(freshest, order, _, _)| (*freshest, *order));
        for (_, _, _, members) in places {
            if out.len() >= budget {
                break;
            }
            if let Some(server) = members.iter().min_by_key(|s| mix(seed, &s.name)) {
                out.push((server.name.clone(), addresses[&server.name]));
            }
        }
        out
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredPing {
    /// None: probed and unreachable.
    ms: Option<u32>,
    at: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    next: u64,
    measured: Option<u64>,
    pings: BTreeMap<String, StoredPing>,
}
const STORE_VERSION: u32 = 1;
const MAX_STORED: u64 = 256 * 1024;
const MAX_ENTRIES: usize = 2048;

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Latency results and the probe cooldown, kept across launches so a restart
/// does not probe again. Holds server names, milliseconds and times only: no
/// account data and no addresses. Lives in the user's config directory.
#[derive(Clone, Debug)]
pub struct LatencyStore {
    path: PathBuf,
}
impl LatencyStore {
    pub fn in_directory(directory: &Path) -> Self {
        Self {
            path: directory.join("latency.json"),
        }
    }
    /// Restore into `cache` (replacing its results). Unreadable, oversized,
    /// symlinked or invalid files, entries from the future and entries older
    /// than `MAX_AGE` are ignored; a cooldown never extends past `MIN_INTERVAL`.
    pub fn load(&self, cache: &mut LatencyCache, now: Instant) {
        self.load_at(cache, now, now_unix());
    }
    pub fn load_at(&self, cache: &mut LatencyCache, now: Instant, wall: u64) {
        let Some(stored) = self.read() else { return };
        let instant = |t: u64| -> Option<Instant> {
            if t > wall.saturating_add(60) {
                return None;
            }
            let age = Duration::from_secs(wall.saturating_sub(t));
            if age >= MAX_AGE {
                return None;
            }
            now.checked_sub(age)
        };
        cache.pings = stored
            .pings
            .into_iter()
            .filter_map(|(name, p)| {
                let ping = match p.ms {
                    Some(ms) => Ping::Measured(ms.max(1)),
                    None => Ping::Unreachable,
                };
                Some((name, (ping, instant(p.at)?)))
            })
            .collect();
        cache.measured_at = stored.measured.and_then(instant);
        // A clock moved backwards cannot leave a cooldown longer than intended.
        cache.next = (stored.next > wall && stored.next <= wall.saturating_add(60 * 60))
            .then(|| now + Duration::from_secs(stored.next - wall).min(MIN_INTERVAL));
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
        let valid = stored.version == STORE_VERSION
            && stored.pings.len() <= MAX_ENTRIES
            && stored.pings.iter().all(|(name, p)| {
                !name.is_empty()
                    && name.len() <= 64
                    && name
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_. ".contains(&c))
                    && p.ms.is_none_or(|ms| ms <= 60_000)
            });
        valid.then_some(stored)
    }
    pub fn save(&self, cache: &LatencyCache, now: Instant) -> anyhow::Result<()> {
        self.save_at(cache, now, now_unix())
    }
    pub fn save_at(&self, cache: &LatencyCache, now: Instant, wall: u64) -> anyhow::Result<()> {
        use anyhow::Context;
        let unix = |t: Instant| wall.saturating_sub(now.saturating_duration_since(t).as_secs());
        let stored = Stored {
            version: STORE_VERSION,
            next: cache
                .next
                .map(|t| wall + t.saturating_duration_since(now).as_secs())
                .unwrap_or(0),
            measured: cache.measured_at.map(unix),
            pings: cache
                .pings
                .iter()
                .take(MAX_ENTRIES)
                .map(|(name, (ping, at))| {
                    let ms = match ping {
                        Ping::Measured(ms) => Some((*ms).min(60_000)),
                        _ => None,
                    };
                    (name.clone(), StoredPing { ms, at: unix(*at) })
                })
                .collect(),
        };
        let directory = self
            .path
            .parent()
            .context("Latency store has no directory")?;
        std::fs::create_dir_all(directory)?;
        anyhow::ensure!(
            std::fs::symlink_metadata(directory)?.is_dir(),
            "Latency directory must not be a symlink"
        );
        let mut temp = tempfile::NamedTempFile::new_in(directory)?;
        temp.write_all(&serde_json::to_vec(&stored)?)?;
        temp.as_file().sync_all()?;
        temp.persist(&self.path)
            .context("Unable to save latency cache")?;
        Ok(())
    }
}
