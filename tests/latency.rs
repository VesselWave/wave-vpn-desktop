// SPDX-License-Identifier: GPL-3.0-or-later
//! Latency probing policy with a fake prober and a fake clock. No sockets.
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use wave_protocol::{
    latency::{
        measure, targets, LatencyCache, Outcome, Ping, Prober, MAX_TIMEOUT, MIN_INTERVAL, PARALLEL,
        RETRY_INTERVAL,
    },
    parse_manifest, Server,
};

fn servers(count: usize) -> Vec<Server> {
    let template =
        parse_manifest(include_str!("../protocol-data/fixtures/eddie-manifest.xml")).unwrap();
    (0..count)
        .map(|i| {
            let mut s = template.servers[0].clone();
            s.name = format!("S{i:03}");
            s.entries = vec![
                format!("2001:db8::{i:x}").parse().unwrap(),
                format!("10.{}.{}.1", i / 250, i % 250).parse().unwrap(),
                format!("10.{}.{}.2", i / 250, i % 250).parse().unwrap(),
            ];
            s
        })
        .collect()
}

/// Answers from a table; tracks concurrency and every target it was asked about.
#[derive(Default)]
struct Fake {
    answers: HashMap<IpAddr, Option<Duration>>,
    default: Option<Duration>,
    active: AtomicUsize,
    peak: AtomicUsize,
    probed: Mutex<Vec<SocketAddr>>,
    cancel_after: Option<(usize, &'static AtomicBool)>,
}
impl Prober for Fake {
    fn probe(&self, target: SocketAddr, _timeout: Duration) -> Option<Duration> {
        let now = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(now, Ordering::SeqCst);
        let count = {
            let mut probed = self.probed.lock().unwrap();
            probed.push(target);
            probed.len()
        };
        if let Some((after, flag)) = self.cancel_after {
            if count >= after {
                flag.store(true, Ordering::SeqCst);
            }
        }
        // Overlap probes so a missing concurrency bound would show up.
        std::thread::sleep(Duration::from_millis(2));
        self.active.fetch_sub(1, Ordering::SeqCst);
        self.answers
            .get(&target.ip())
            .copied()
            .unwrap_or(self.default)
    }
}

#[test]
fn one_target_per_server_on_its_first_ipv4_entry() {
    let mut list = servers(3);
    list[2].entries.retain(IpAddr::is_ipv6);
    let t = targets(&list);
    assert_eq!(t.len(), 2, "IPv6-only servers are not probed");
    assert_eq!(t[0], ("S000".into(), "10.0.0.1:443".parse().unwrap()));
    assert_eq!(t[1], ("S001".into(), "10.0.1.1:443".parse().unwrap()));
}

#[test]
fn probes_are_bounded_in_parallel_and_sent_once_per_server() {
    let list = servers(250);
    let fake = Fake {
        default: Some(Duration::from_millis(40)),
        ..Default::default()
    };
    let never = AtomicBool::new(false);
    // Asking for 64 workers still runs at most PARALLEL probes at once.
    let Outcome::Complete(pings) =
        measure(&targets(&list), &fake, Duration::from_secs(3), 64, &never)
    else {
        panic!("expected complete measurement");
    };
    assert_eq!(pings.len(), 250);
    assert!(fake.peak.load(Ordering::SeqCst) <= PARALLEL);
    let mut probed = fake.probed.lock().unwrap().clone();
    assert_eq!(probed.len(), 250);
    probed.sort();
    probed.dedup();
    assert_eq!(probed.len(), 250, "no server probed twice");
}

#[test]
fn unreachable_is_never_zero_and_instant_answers_are_at_least_one_ms() {
    let list = servers(4);
    let ip = |i: usize| targets(&list)[i].1.ip();
    let fake = Fake {
        answers: HashMap::from([
            (ip(0), Some(Duration::ZERO)),
            (ip(1), Some(Duration::from_millis(87))),
            (ip(2), None),
            // Slower than the (clamped) timeout: did not answer in time.
            (ip(3), Some(MAX_TIMEOUT + Duration::from_secs(1))),
        ]),
        ..Default::default()
    };
    let never = AtomicBool::new(false);
    let Outcome::Complete(pings) =
        measure(&targets(&list), &fake, Duration::from_secs(60), 4, &never)
    else {
        panic!("expected complete measurement");
    };
    assert_eq!(pings["S000"], Ping::Measured(1));
    assert_eq!(pings["S001"], Ping::Measured(87));
    assert_eq!(pings["S002"], Ping::Unreachable);
    assert_eq!(pings["S003"], Ping::Unreachable);
    assert_eq!(
        Ping::Unreachable.cost(),
        wave_protocol::latency::UNREACHABLE_COST
    );
    assert!(Ping::Unreachable.order() > Ping::Unknown.order());
    assert!(Ping::Unknown.order() > Ping::Measured(9_999).order());
}

#[test]
fn nothing_answering_means_offline_not_every_server_unreachable() {
    let fake = Fake::default();
    let never = AtomicBool::new(false);
    assert_eq!(
        measure(
            &targets(&servers(20)),
            &fake,
            Duration::from_secs(1),
            4,
            &never
        ),
        Outcome::Offline
    );
}

#[test]
fn cancellation_stops_new_probes_and_drops_partial_results() {
    static CANCEL: AtomicBool = AtomicBool::new(false);
    let fake = Fake {
        default: Some(Duration::from_millis(10)),
        cancel_after: Some((5, &CANCEL)),
        ..Default::default()
    };
    let outcome = measure(
        &targets(&servers(250)),
        &fake,
        Duration::from_secs(1),
        4,
        &CANCEL,
    );
    assert_eq!(outcome, Outcome::Cancelled);
    assert!(fake.probed.lock().unwrap().len() < 5 + PARALLEL);
}

#[test]
fn cache_is_reused_for_the_minimum_interval_and_never_probes_with_a_tunnel() {
    let t0 = Instant::now();
    let mut cache = LatencyCache::default();
    assert!(cache.due(t0, false));
    assert!(!cache.due(t0, true), "never while connecting or connected");
    let run = cache.begin(t0);
    assert!(!cache.due(t0, false), "one run at a time");
    let done = t0 + Duration::from_secs(20);
    assert!(cache.finish(
        run,
        done,
        Outcome::Complete([("A".into(), Ping::Measured(30))].into())
    ));
    assert_eq!(cache.measured_at(), Some(done));
    assert!(!cache.due(done + MIN_INTERVAL - Duration::from_secs(1), false));
    assert!(cache.due(done + MIN_INTERVAL, false));
    assert!(!cache.due(done + MIN_INTERVAL, true));
}

#[test]
fn cancelled_or_stale_results_are_ignored_and_failures_back_off() {
    let t0 = Instant::now();
    let mut cache = LatencyCache::default();
    let first = cache.begin(t0);
    cache.cancel();
    assert!(!cache.running());
    assert!(!cache.finish(
        first,
        t0,
        Outcome::Complete([("A".into(), Ping::Measured(5))].into())
    ));
    assert_eq!(cache.get("A"), Ping::Unknown, "stale run cannot write");
    assert!(!cache.due(t0, false), "a cancelled run still waits");
    assert!(cache.due(t0 + RETRY_INTERVAL, false));

    let second = cache.begin(t0 + RETRY_INTERVAL);
    cache.finish(
        second,
        t0 + RETRY_INTERVAL,
        Outcome::Complete([("A".into(), Ping::Measured(5))].into()),
    );
    let later = t0 + RETRY_INTERVAL + MIN_INTERVAL;
    let third = cache.begin(later);
    assert!(!cache.finish(third, later, Outcome::Offline));
    assert_eq!(cache.get("A"), Ping::Measured(5), "offline keeps the cache");
    assert!(!cache.due(later + RETRY_INTERVAL - Duration::from_secs(1), false));
    assert!(cache.due(later + RETRY_INTERVAL, false));

    cache.clear();
    assert_eq!(cache.get("A"), Ping::Unknown);
    assert!(cache.due(later, false));
}

#[test]
fn new_servers_rank_at_the_median_and_unreachable_ones_last() {
    let t0 = Instant::now();
    let mut cache = LatencyCache::default();
    assert_eq!(cache.ranking("New"), Ping::Unknown);
    let run = cache.begin(t0);
    cache.finish(
        run,
        t0,
        Outcome::Complete(
            [
                ("A".into(), Ping::Measured(10)),
                ("B".into(), Ping::Measured(50)),
                ("C".into(), Ping::Measured(90)),
                ("D".into(), Ping::Unreachable),
            ]
            .into(),
        ),
    );
    assert_eq!(cache.get("New"), Ping::Unknown, "display stays honest");
    assert_eq!(cache.ranking("New"), Ping::Measured(50));
    assert_eq!(cache.ranking("D"), Ping::Unreachable);
}

/// A day of launches, sign-ins and refreshes every 15 s (the old minimum),
/// with the tunnel up half of the time, starts at most one run per interval
/// and none while connected.
#[test]
fn a_day_of_frequent_refreshes_measures_rarely() {
    let t0 = Instant::now();
    let mut cache = LatencyCache::default();
    let mut runs = 0;
    let mut connected_runs = 0;
    for step in 0..(24 * 3600 / 15) {
        let now = t0 + Duration::from_secs(step * 15);
        let connected = (step * 15 / 3600) % 2 == 1;
        if cache.due(now, connected) {
            runs += 1;
            connected_runs += usize::from(connected);
            let run = cache.begin(now);
            cache.finish(
                run,
                now + Duration::from_secs(10),
                Outcome::Complete([("A".into(), Ping::Measured(20))].into()),
            );
        }
    }
    assert_eq!(connected_runs, 0);
    assert!(
        runs <= 24,
        "{runs} runs; at most one per 30 min while disconnected"
    );
}

// Two app instances sharing one config directory share one probe budget.
#[test]
fn recheck_second_instance_is_not_admitted_during_another_cooldown() {
    let dir = tempfile::tempdir().unwrap();
    let first = wave_protocol::latency::LatencyStore::in_directory(dir.path());
    let second = wave_protocol::latency::LatencyStore::in_directory(dir.path());
    let now = Instant::now();
    let wall = 1_000_000;
    let mut a = LatencyCache::default();
    let mut b = LatencyCache::default();
    assert!(first
        .admit_at(&mut a, now, wall, |c| c.begin(now))
        .is_some());
    assert!(second
        .admit_at(&mut b, now, wall + 10, |_| panic!("must not start"))
        .is_none());
    assert!(
        b.next().is_some_and(|t| t > now),
        "adopts the running cooldown"
    );
    // After the cooldown expires the other instance may run.
    let later = wall + RETRY_INTERVAL.as_secs() + 1;
    assert!(second
        .admit_at(&mut b, now, later, |c| c.begin(now))
        .is_some());
}
