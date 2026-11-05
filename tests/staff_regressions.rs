// SPDX-License-Identifier: GPL-3.0-or-later
//! Independently derived from AirVPN staff comment 268064; fake clocks/probes only.
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use wave_protocol::{
    checks::{self, Verdict},
    latency::{self, LatencyCache, Outcome, Ping, Prober},
    schedule::{self, RefreshSchedule},
};

#[test]
fn staff_observed_mismatch_cannot_be_erased_by_a_retry() {
    for later in [Verdict::Unavailable, Verdict::Verified] {
        let mut calls = 0;
        let result = checks::verify_at_connect(
            3,
            &mut || {
                calls += 1;
                if calls == 1 {
                    Verdict::Failed("observed DNS mismatch")
                } else {
                    later
                }
            },
            &mut |_| (),
            &|| false,
        );
        assert!(result.stops_connection(), "mismatch became {result:?}");
    }
}

#[test]
fn staff_minimum_refresh_interval_survives_jitter() {
    let now = Instant::now();
    let floor = schedule::interval(15, None);
    let mut s = RefreshSchedule::new(now);
    s.succeeded(now, floor, 0.0);
    assert!(!s.due(now + Duration::from_secs(299)));
    assert!(s.next() >= now + Duration::from_secs(300));
}

#[test]
fn staff_cancel_during_verification_pause_prevents_another_request() {
    let stop = AtomicBool::new(false);
    let mut requests = 0;
    let mut pauses = 0;
    let verdict = checks::verify_at_connect(
        99,
        &mut || {
            requests += 1;
            Verdict::Unavailable
        },
        &mut |_| {
            pauses += 1;
            stop.store(true, Ordering::SeqCst);
        },
        &|| stop.load(Ordering::SeqCst),
    );
    assert_eq!((verdict, requests, pauses), (Verdict::Stopped, 1, 1));
}

#[test]
fn staff_concurrent_refresh_actions_have_one_admitted_request() {
    // GUI serializes actions; model that ownership explicitly across callers.
    let now = Instant::now();
    let schedule = Mutex::new(RefreshSchedule::new(now));
    let admitted = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..100 {
            scope.spawn(|| {
                let mut s = schedule.lock().unwrap();
                if s.manual_allowed(now) {
                    s.start(now);
                    admitted.fetch_add(1, Ordering::SeqCst);
                }
            });
        }
    });
    assert_eq!(admitted.load(Ordering::SeqCst), 1);
    let mut s = schedule.into_inner().unwrap();
    // Unit 0 is the earliest possible retry (R2: jitter only lengthens it), so
    // this still pins the exact five-minute boundary.
    s.failed(now, Duration::from_secs(1800), 0.0);
    assert!(!s.manual_allowed(now + Duration::from_secs(59)));
    assert!(s.manual_allowed(now + Duration::from_secs(60)));
    assert!(!s.due(now + Duration::from_secs(299)));
    assert!(s.due(now + Duration::from_secs(300)));
}

#[test]
fn staff_expired_cache_remains_usable_during_failed_refresh_and_stale_delivery() {
    let now = Instant::now();
    let mut c = LatencyCache::default();
    let first = c.begin(now);
    assert!(c.finish(
        first,
        now,
        Outcome::Complete(
            [
                ("A".into(), Ping::Measured(70)),
                ("B".into(), Ping::Measured(90))
            ]
            .into()
        )
    ));
    let expiry = now + Duration::from_secs(1800);
    assert!(!c.due(expiry - Duration::from_nanos(1), false));
    assert!(c.due(expiry, false));
    let retry = c.begin(expiry);
    c.cancel();
    assert!(!c.finish(
        retry,
        expiry,
        Outcome::Complete([("A".into(), Ping::Measured(1))].into())
    ));
    assert_eq!(c.get("A"), Ping::Measured(70));
    assert_eq!(c.get("New"), Ping::Unknown);
    assert!(matches!(c.ranking("New"), Ping::Measured(n) if (70..=90).contains(&n)));
    assert!(!c.due(expiry + Duration::from_secs(299), false));
    assert!(c.due(expiry + Duration::from_secs(300), false));
}

#[test]
fn staff_cancellation_drains_only_inflight_probes_and_clamps_timeout() {
    struct Blocking<'a> {
        cancel: &'a AtomicBool,
        calls: AtomicUsize,
    }
    impl Prober for Blocking<'_> {
        fn probe(&self, _: std::net::SocketAddr, timeout: Duration) -> Option<Duration> {
            assert_eq!(timeout, Duration::from_secs(5));
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.cancel.store(true, Ordering::SeqCst);
            Some(Duration::from_millis(10))
        }
    }
    let cancel = AtomicBool::new(false);
    let fake = Blocking {
        cancel: &cancel,
        calls: AtomicUsize::new(0),
    };
    let targets: Vec<_> = (0..250)
        .map(|n| (format!("S{n}"), "192.0.2.1:443".parse().unwrap()))
        .collect();
    assert_eq!(
        latency::measure(&targets, &fake, Duration::from_secs(3600), 999, &cancel),
        Outcome::Cancelled
    );
    assert!((1..=4).contains(&fake.calls.load(Ordering::SeqCst)));
}

#[test]
fn staff_network_lock_must_exempt_latest_bootstrap_ip_port() {
    // This public policy entry point is what Linux set_firewall calls. A latest
    // manifest can replace all embedded addresses; merely saving it is insufficient.
    // R3 fix: the policy now requires the stored latest exemptions, which the
    // desktop passes to the helper (see the GUI and transport wiring tests).
    let directory = tempfile::tempdir().unwrap();
    let store = wave_protocol::BootstrapStore::in_directory(directory.path());
    let mut latest = wave_protocol::Bootstrap::eddie();
    latest.urls = vec!["http://45.90.113.77:8080".into()];
    store.save(&latest).unwrap();
    assert_eq!(store.load(), Some(latest));
    let mut options = wave_protocol::options::Options::default();
    options.set("allow_lan", "false".into());
    let rules =
        wave_protocol::firewall::policy(&options, None, true, &store.exemptions(), Some(1000))
            .unwrap();
    assert!(
        rules.contains("ip daddr 45.90.113.77 tcp dport 8080 accept"),
        "rotated bootstrap is blocked under default-drop policy"
    );
}
