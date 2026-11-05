// SPDX-License-Identifier: GPL-3.0-or-later
//! Server latency: few probes in parallel, cached, never while a tunnel is up.
use crate::Server;
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

/// Probes in flight at once, instead of one per server in large batches.
pub const PARALLEL: usize = 4;
/// A complete measurement is reused for this long, whatever triggers a refresh.
pub const MIN_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// After an interrupted or failed run (offline, firewalled), wait this long.
pub const RETRY_INTERVAL: Duration = Duration::from_secs(5 * 60);
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
/// TCP connect to the entry address. The connection is closed immediately.
pub struct TcpProber;
impl Prober for TcpProber {
    fn probe(&self, target: SocketAddr, timeout: Duration) -> Option<Duration> {
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
    pings: BTreeMap<String, Ping>,
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
    /// Returns whether the result was accepted.
    pub fn finish(&mut self, generation: u64, now: Instant, outcome: Outcome) -> bool {
        if generation != self.generation || !self.running {
            return false;
        }
        self.running = false;
        match outcome {
            Outcome::Complete(pings) => {
                self.pings = pings;
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
    pub fn insert(&mut self, name: String, ping: Ping) {
        self.pings.insert(name, ping);
    }
    /// What the user sees.
    pub fn get(&self, name: &str) -> Ping {
        self.pings.get(name).copied().unwrap_or(Ping::Unknown)
    }
    /// What the ranking uses: a server added since the last run is neither
    /// favoured nor punished; it is ranked at the median measured latency.
    pub fn ranking(&self, name: &str) -> Ping {
        match self.get(name) {
            Ping::Unknown => {
                let mut measured: Vec<u32> = self
                    .pings
                    .values()
                    .filter_map(|p| match p {
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
}
