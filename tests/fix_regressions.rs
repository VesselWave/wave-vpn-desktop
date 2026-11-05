// SPDX-License-Identifier: GPL-3.0-or-later
//! Guards added for the staff regression findings R1-R4 and probe load.
//! Fake probes, clocks and stores only; the one socket test uses loopback.
use std::{
    cell::RefCell,
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use wave_protocol::{
    bootstrap::{Exemption, MAX_EXEMPTIONS},
    checks::{self, Outcome as Check, ProviderCheck, Results, Verdict, VerificationFailed},
    latency::{
        self, LatencyCache, LatencyStore, Outcome, Ping, Prober, TcpProber, MAX_AGE, MIN_INTERVAL,
        RETRY_INTERVAL, SAMPLE_BUDGET,
    },
    options::Options,
    parse_manifest,
    schedule::{self, RefreshSchedule, MIN_AUTOMATIC},
    transport::{failure_event, LockInput, PersistentLock},
    Server,
};

// ---- R1: an observed mismatch is terminal and latched.

struct Scripted {
    tunnel: Vec<Option<bool>>,
    dns: Vec<Option<bool>>,
    requests: usize,
}
impl checks::Probe for Scripted {
    fn tunnel(&mut self, local: IpAddr) -> anyhow::Result<serde_json::Value> {
        self.requests += 1;
        match self.tunnel.remove(0) {
            Some(true) => Ok(serde_json::json!({"ip": local.to_string()})),
            Some(false) => Ok(serde_json::json!({"ip": "198.51.100.99"})),
            None => anyhow::bail!("service unavailable"),
        }
    }
    fn resolve(&mut self, _: &str) {}
    fn dns(&mut self, _: IpAddr) -> anyhow::Result<serde_json::Value> {
        self.requests += 1;
        match self.dns.remove(0) {
            Some(true) => Ok(serde_json::json!({"dns": "token"})),
            Some(false) => Ok(serde_json::json!({"dns": "someone-else"})),
            None => anyhow::bail!("service unavailable"),
        }
    }
}
fn check() -> ProviderCheck {
    ProviderCheck {
        server: "Alpha".into(),
        exits: vec!["198.51.100.1".parse().unwrap()],
        domain: "airservers.org:89".into(),
        dns_query: "{hash}.checkdns.airvpn.org".into(),
    }
}

#[test]
fn fix_dns_mismatch_stops_after_one_pass_with_failed_status_kept() {
    let probe = RefCell::new(Scripted {
        tunnel: vec![Some(true), Some(true), Some(true)],
        dns: vec![Some(false), None, Some(true)],
        requests: 0,
    });
    let results = RefCell::new(Results::default());
    let published = RefCell::new(vec![]);
    let locals = ["10.1.2.3".parse().unwrap()];
    let mut passes = 0;
    let verdict = checks::verify_at_connect(
        3,
        &mut || {
            passes += 1;
            checks::verify_once(
                &check(),
                &locals,
                &mut *probe.borrow_mut(),
                "token",
                &mut results.borrow_mut(),
                &mut |r| published.borrow_mut().push(r.clone()),
                &|| false,
            )
        },
        &mut |_| panic!("no retry after an observed mismatch"),
        &|| false,
    );
    assert!(verdict.stops_connection());
    assert_eq!(passes, 1);
    assert_eq!(results.borrow().dns, Check::Failed);
    assert_eq!(published.borrow().last().unwrap().dns, Check::Failed);
    assert!(results.borrow().failure().is_some());
}

#[test]
fn fix_a_latched_failure_cannot_be_cleared_by_a_later_pass() {
    for later in [Some(true), None] {
        let mut probe = Scripted {
            tunnel: vec![later],
            dns: vec![later],
            requests: 0,
        };
        let mut results = Results {
            ipv4: Check::Failed,
            ..Default::default()
        };
        let verdict = checks::verify_once(
            &check(),
            &["10.1.2.3".parse().unwrap()],
            &mut probe,
            "token",
            &mut results,
            &mut |_| (),
            &|| false,
        );
        assert!(verdict.stops_connection(), "{later:?} gave {verdict:?}");
        assert_eq!(results.ipv4, Check::Failed);
        assert_eq!(probe.requests, 0, "no requests once already failed");
    }
}

#[test]
fn fix_status_merge_keeps_observed_failures_and_updates_the_rest() {
    let mut shown = Results::default();
    shown.merge(&Results {
        dns: Check::Failed,
        ipv4: Check::Verified,
        ..Default::default()
    });
    shown.merge(&Results {
        dns: Check::Unavailable,
        ipv4: Check::Unavailable,
        ipv6: Check::Verified,
    });
    assert_eq!(
        (shown.ipv4, shown.ipv6, shown.dns),
        (Check::Unavailable, Check::Verified, Check::Failed)
    );
    assert!(shown.failure().is_some());
}

#[test]
fn fix_unavailable_is_still_retried_and_never_stops() {
    let mut calls = 0;
    let verdict = checks::verify_at_connect(
        3,
        &mut || {
            calls += 1;
            Verdict::Unavailable
        },
        &mut |_| (),
        &|| false,
    );
    assert_eq!((verdict, calls), (Verdict::Unavailable, 3));
    assert!(!verdict.stops_connection());
}

#[test]
fn fix_verification_failure_is_reported_as_not_retryable() {
    let failed = anyhow::Error::new(VerificationFailed("DNS"));
    assert_eq!(
        failure_event(true, &failed),
        serde_json::json!({"event":"failure","retryable":false,"category":"verification"})
    );
    let other = anyhow::anyhow!("WireGuard handshake expired");
    assert_eq!(failure_event(true, &other)["retryable"], true);
    assert_eq!(failure_event(false, &other)["category"], "configuration");
}

// ---- R2: strict minimum and next_update, positive jitter only.

#[test]
fn fix_refresh_is_never_scheduled_before_the_floor_or_next_update() {
    let now = Instant::now();
    for hint in [None, Some(1), Some(30), Some(90)] {
        let interval = schedule::interval(15, hint);
        assert!(interval >= MIN_AUTOMATIC);
        assert!(interval >= Duration::from_secs(u64::from(hint.unwrap_or(0)) * 60));
        for unit in [0.0, 0.25, 0.5, 1.0, -3.0, 7.0, f64::NAN] {
            let mut s = RefreshSchedule::new(now);
            s.succeeded(now, interval, unit);
            assert!(s.next() >= now + interval, "{hint:?} {unit}");
            assert!(s.next() <= now + interval.mul_f64(1.2));
            assert!(!s.due(now + interval - Duration::from_nanos(1)));
            for failures in 1..8 {
                let start = now + Duration::from_secs(failures * 10_000);
                s.start(start);
                s.failed(start, interval, unit);
                assert!(s.next() >= start + MIN_AUTOMATIC, "retry {failures} {unit}");
            }
        }
    }
    assert_eq!(
        schedule::jitter_above(Duration::from_secs(300), 0.0),
        Duration::from_secs(300)
    );
    assert_eq!(
        schedule::jitter_above(Duration::from_secs(300), 1.0),
        Duration::from_secs(360)
    );
}

#[test]
fn fix_a_failed_manual_refresh_does_not_pull_the_schedule_earlier() {
    let now = Instant::now();
    let mut s = RefreshSchedule::new(now);
    let hint = Duration::from_secs(1800);
    s.succeeded(now, hint, 0.0);
    let manual = now + Duration::from_secs(120);
    s.start(manual);
    s.failed(manual, hint, 0.0);
    assert!(!s.due(now + hint - Duration::from_secs(1)));
    assert!(s.due(now + hint));
}

#[test]
fn fix_account_document_is_added_to_automatic_refresh_only_daily() {
    let t0 = Instant::now();
    assert!(!schedule::account_due(
        t0,
        t0 + Duration::from_secs(6 * 3600)
    ));
    assert!(schedule::account_due(t0, t0 + schedule::ACCOUNT_INTERVAL));
}

// ---- R3: bounded, validated helper inputs for the lock.

#[test]
fn fix_lock_input_accepts_structured_and_legacy_forms_and_rejects_unsafe_exemptions() {
    let options = Options::default();
    let rotated = Exemption {
        ip: "203.0.113.77".parse().unwrap(),
        port: 8080,
    };
    let structured = serde_json::to_vec(&LockInput {
        options: options.clone(),
        bootstrap: vec![rotated],
    })
    .unwrap();
    assert_eq!(
        LockInput::parse(&structured).unwrap().bootstrap,
        vec![rotated]
    );
    let legacy = serde_json::to_vec(&options).unwrap();
    assert!(LockInput::parse(&legacy).unwrap().bootstrap.is_empty());
    assert_eq!(LockInput::parse(b"").unwrap(), LockInput::default());
    for bad in [
        serde_json::json!({"options":{},"bootstrap":[{"ip":"10.0.0.1","port":80}]}),
        serde_json::json!({"options":{},"bootstrap":[{"ip":"203.0.113.77","port":53}]}),
        serde_json::json!({"options":{},"bootstrap":[{"ip":"bootstrap.example","port":80}]}),
        serde_json::json!({"options":{},"bootstrap":[{"ip":"203.0.113.0/24","port":80}]}),
        serde_json::json!({"options":{},"bootstrap":[{"ip":"203.0.113.77","port":80,"proto":"udp"}]}),
        serde_json::json!({"options":{},"bootstrap":"0.0.0.0/0"}),
        serde_json::json!({"options":{"unknown":"x"},"bootstrap":[]}),
        serde_json::json!({"options":{},"bootstrap":[],"extra":true}),
    ] {
        assert!(
            LockInput::parse(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "accepted {bad}"
        );
    }
    let many: Vec<_> = (0..=MAX_EXEMPTIONS as u16)
        .map(|p| serde_json::json!({"ip":"203.0.113.77","port":2000 + p}))
        .collect();
    let too_many = serde_json::json!({"options":{},"bootstrap":many});
    assert!(LockInput::parse(&serde_json::to_vec(&too_many).unwrap()).is_err());
}

#[test]
fn fix_persistent_lock_restores_rotated_exemptions_after_restart() {
    let rotated = Exemption {
        ip: "2001:db8::77".parse().unwrap(),
        port: 8443,
    };
    let saved = serde_json::to_vec(&PersistentLock {
        options: Options::default(),
        enabled: true,
        bootstrap: vec![rotated],
    })
    .unwrap();
    // Boot: lock-restore parses the root-only file and applies the policy.
    let restored = PersistentLock::parse(&saved).unwrap();
    let rules = wave_protocol::firewall::policy(&restored.options, None, true, &restored.bootstrap)
        .unwrap();
    assert!(rules.contains("ip6 daddr 2001:db8::77 tcp dport 8443 accept"));
    // Pre-upgrade files have no list; damaged entries are dropped, never fatal.
    let legacy = serde_json::json!({"options":{},"enabled":true});
    assert!(PersistentLock::parse(&serde_json::to_vec(&legacy).unwrap())
        .unwrap()
        .bootstrap
        .is_empty());
    let damaged = serde_json::json!({"options":{},"enabled":true,
        "bootstrap":[{"ip":"127.0.0.1","port":80},{"ip":"203.0.113.9","port":80}]});
    assert_eq!(
        PersistentLock::parse(&serde_json::to_vec(&damaged).unwrap())
            .unwrap()
            .bootstrap,
        vec![Exemption {
            ip: "203.0.113.9".parse().unwrap(),
            port: 80
        }]
    );
}

#[test]
fn fix_worker_request_carries_exemptions_and_rejects_unknown_fields() {
    let request = serde_json::json!({
        "profile": {"private_key":"x","preshared_key":"x","public_key":"x","addresses":[],
            "dns":[],"endpoint":"192.0.2.1","port":1637},
        "options": {}, "bootstrap": [{"ip":"203.0.113.77","port":8080}]
    });
    let r = serde_json::from_value::<wave_protocol::transport::Request>(request.clone()).unwrap();
    assert_eq!(r.bootstrap[0].port, 8080);
    let mut extra = request;
    extra["allow"] = serde_json::json!("0.0.0.0/0");
    assert!(serde_json::from_value::<wave_protocol::transport::Request>(extra).is_err());
}

// ---- Probe load: bounded representative sample, persisted cooldown.

fn fleet(locations: usize, per_location: usize) -> Vec<Server> {
    let template =
        parse_manifest(include_str!("../protocol-data/fixtures/eddie-manifest.xml")).unwrap();
    let mut out = vec![];
    for l in 0..locations {
        for n in 0..per_location {
            let mut s = template.servers[0].clone();
            s.name = format!("Srv{}x{}", l, n);
            s.country = format!("c{}", l % 40);
            s.location = format!("City{l}");
            s.entries = vec![format!("192.0.2.{}", (l * per_location + n) % 250 + 1)
                .parse()
                .unwrap()];
            out.push(s);
        }
    }
    out
}

#[test]
fn fix_sample_is_bounded_one_per_location_and_not_alphabetical() {
    let servers = fleet(80, 3); // 240 servers in 80 locations
    let cache = LatencyCache::default();
    let mut firsts = std::collections::BTreeSet::new();
    let mut alphabetical = 0;
    for seed in 0..20u64 {
        let sample = cache.sample(&servers, SAMPLE_BUDGET, &[], seed);
        assert_eq!(sample.len(), SAMPLE_BUDGET);
        let places: std::collections::BTreeSet<_> = sample
            .iter()
            .map(|(n, _)| n.split('x').next().unwrap().to_owned())
            .collect();
        assert_eq!(places.len(), sample.len(), "one server per location");
        let mut names: Vec<_> = servers.iter().map(|s| s.name.clone()).collect();
        names.sort();
        let mut chosen: Vec<_> = sample.iter().map(|(n, _)| n.clone()).collect();
        chosen.sort();
        alphabetical += usize::from(chosen == names[..SAMPLE_BUDGET]);
        firsts.insert(sample[0].0.clone());
    }
    assert_eq!(alphabetical, 0);
    assert!(firsts.len() > 5, "seeds spread the order: {firsts:?}");
}

#[test]
fn fix_sample_prefers_unmeasured_locations_and_keeps_explicit_selection() {
    let servers = fleet(40, 2);
    let mut cache = LatencyCache::default();
    cache.set_locations(&servers);
    let t0 = Instant::now();
    let run = cache.begin(t0);
    let first = cache.sample(&servers, SAMPLE_BUDGET, &[], 1);
    let measured: BTreeMap<_, _> = first
        .iter()
        .map(|(n, _)| (n.clone(), Ping::Measured(40)))
        .collect();
    assert!(cache.finish(run, t0, Outcome::Complete(measured)));
    // The next run covers the 8 locations not measured yet before any repeat.
    let second = cache.sample(&servers, SAMPLE_BUDGET, &[], 2);
    let place = |n: &str| n.split('x').next().unwrap().to_owned();
    let before: std::collections::BTreeSet<_> = first.iter().map(|(n, _)| place(n)).collect();
    for (name, _) in &second[..8] {
        assert!(!before.contains(&place(name)), "{name} repeated too early");
    }
    // An explicitly selected server is probed even if its location is fresh,
    // and the budget still holds.
    let sibling = |n: &str| match n.strip_suffix("x0") {
        Some(base) => format!("{base}x1"),
        None => format!("{}x0", n.strip_suffix("x1").unwrap()),
    };
    let explicit = sibling(&first[0].0);
    let third = cache.sample(&servers, SAMPLE_BUDGET, &[explicit.as_str()], 3);
    assert_eq!(third[0].0, explicit);
    assert!(third.len() <= SAMPLE_BUDGET);
    // Unprobed servers show their location's result; ranking falls back to it.
    let other = sibling(&first[1].0);
    assert_eq!(cache.get(&other), Ping::Measured(40));
    assert_eq!(cache.ranking(&other), Ping::Measured(40));
}

#[test]
fn fix_whole_fleet_is_not_probed_on_every_launch() {
    struct Count(std::sync::atomic::AtomicUsize);
    impl Prober for Count {
        fn probe(&self, _: SocketAddr, _: Duration) -> Option<Duration> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Some(Duration::from_millis(30))
        }
    }
    let servers = fleet(80, 3);
    let dir = tempfile::tempdir().unwrap();
    let store = LatencyStore::in_directory(dir.path());
    let prober = Count(Default::default());
    let never = AtomicBool::new(false);
    let wall0 = 1_800_000_000u64;
    let t0 = Instant::now();
    // Ten launches, one every five minutes, each with a fresh process cache.
    for launch in 0..10u64 {
        let now = t0 + Duration::from_secs(launch * 300);
        let wall = wall0 + launch * 300;
        let mut cache = LatencyCache::default();
        store.load_at(&mut cache, now, wall);
        if cache.due(now, false) {
            let targets = cache.sample(&servers, SAMPLE_BUDGET, &[], launch);
            let run = cache.begin(now);
            store.save_at(&cache, now, wall).unwrap();
            let outcome = latency::measure(&targets, &prober, Duration::from_secs(1), 4, &never);
            cache.finish(run, now, outcome);
            store.save_at(&cache, now, wall).unwrap();
        }
    }
    // 50 minutes: one run at launch 0 and one after the 30-minute cooldown.
    assert_eq!(prober.0.load(Ordering::SeqCst), 2 * SAMPLE_BUDGET);
    let text = std::fs::read_to_string(dir.path().join("latency.json")).unwrap();
    assert!(!text.contains("192.0.2."), "no addresses persisted");
    assert!(!text.contains("login") && !text.contains("key"));
}

#[test]
fn fix_persisted_cache_ignores_stale_future_and_overlong_entries() {
    let dir = tempfile::tempdir().unwrap();
    let store = LatencyStore::in_directory(dir.path());
    let t0 = Instant::now() + Duration::from_secs(48 * 3600);
    let wall = 1_800_000_000u64;
    let mut cache = LatencyCache::default();
    let run = cache.begin(t0);
    cache.finish(
        run,
        t0,
        Outcome::Complete(
            [
                ("A".into(), Ping::Measured(20)),
                ("B".into(), Ping::Unreachable),
            ]
            .into(),
        ),
    );
    store.save_at(&cache, t0, wall).unwrap();
    // Restarted 10 minutes later: results and the cooldown survive.
    let mut later = LatencyCache::default();
    let now = t0 + Duration::from_secs(600);
    store.load_at(&mut later, now, wall + 600);
    assert_eq!(later.get("A"), Ping::Measured(20));
    assert_eq!(later.get("B"), Ping::Unreachable);
    assert!(!later.due(now, false));
    assert!(later.due(now + MIN_INTERVAL - Duration::from_secs(600), false));
    // A day later the results are forgotten and probing may resume.
    let mut old = LatencyCache::default();
    store.load_at(&mut old, now, wall + MAX_AGE.as_secs() + 1);
    assert_eq!(old.get("A"), Ping::Unknown);
    assert!(old.due(now, false));
    // Clock moved back an hour: nothing from the future is trusted.
    let mut back = LatencyCache::default();
    store.load_at(&mut back, now, wall - 3600);
    assert_eq!(back.get("A"), Ping::Unknown);
    assert!(back.due(now, false));
    // A cancelled run's cooldown persists too, at most RETRY_INTERVAL here.
    let mut c = LatencyCache::default();
    c.begin(t0);
    c.cancel();
    store.save_at(&c, t0, wall).unwrap();
    let mut r = LatencyCache::default();
    store.load_at(&mut r, t0, wall);
    assert!(!r.due(t0 + RETRY_INTERVAL - Duration::from_secs(1), false));
    assert!(r.due(t0 + RETRY_INTERVAL, false));
    // Tampered files are ignored.
    std::fs::write(dir.path().join("latency.json"), b"{\"version\":1,\"next\":0,\"measured\":null,\"pings\":{\"<script>\":{\"ms\":1,\"at\":1}}}").unwrap();
    let mut t = LatencyCache::default();
    store.load_at(&mut t, t0, wall);
    assert_eq!(t.get("<script>"), Ping::Unknown);
}

#[test]
fn fix_tcp_probe_measures_loopback_and_honours_cancellation() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let target = listener.local_addr().unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let prober = TcpProber {
        cancel: cancel.clone(),
    };
    assert!(prober.probe(target, Duration::from_secs(2)).is_some());
    drop(listener);
    assert!(prober.probe(target, Duration::from_secs(2)).is_none());
    cancel.store(true, Ordering::SeqCst);
    let start = Instant::now();
    assert!(prober.probe(target, Duration::from_secs(5)).is_none());
    assert!(start.elapsed() < Duration::from_millis(200));
}

#[test]
fn recheck_mismatch_is_terminal_at_each_retry_position_even_when_cancel_races() {
    use std::cell::Cell;
    for position in 0..3 {
        for later in [Verdict::Verified, Verdict::Unavailable, Verdict::Stopped] {
            for cancel_on_failure in [false, true] {
                let cancelled = Cell::new(false);
                let mut calls = 0;
                let mut pauses = 0;
                let verdict = checks::verify_at_connect(
                    u64::MAX,
                    &mut || {
                        let current = calls;
                        calls += 1;
                        if current < position {
                            Verdict::Unavailable
                        } else if current == position {
                            cancelled.set(cancel_on_failure);
                            Verdict::Failed("independent observed mismatch")
                        } else {
                            later
                        }
                    },
                    &mut |_| pauses += 1,
                    &|| cancelled.get(),
                );
                assert!(
                    verdict.stops_connection(),
                    "position={position} later={later:?}"
                );
                assert_eq!(calls, position + 1);
                assert_eq!(pauses, position);
            }
        }
    }
}
