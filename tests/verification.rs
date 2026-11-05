// SPDX-License-Identifier: GPL-3.0-or-later
//! Tunnel verification and refresh timing with fake probes and clocks.
use anyhow::{anyhow, Result};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    net::IpAddr,
    time::{Duration, Instant},
};
use wave_protocol::{
    checks::{
        retry_pause, verify_at_connect, verify_once, Due, Outcome, Probe, ProviderCheck, Results,
        Schedule, Verdict, MAX_ATTEMPTS, ROUTE_INTERVAL,
    },
    lifecycle::{retry_delay, retry_delay_jittered},
    schedule::{
        failure_delay, interval, jitter, RefreshSchedule, FAILURE_BASE, FAILURE_CAP, MIN_AUTOMATIC,
        MIN_MANUAL,
    },
};

fn check() -> ProviderCheck {
    ProviderCheck {
        server: "Alpha".into(),
        exits: vec![
            "192.0.2.11".parse().unwrap(),
            "2001:db8::11".parse().unwrap(),
        ],
        domain: "airservers.org:89".into(),
        dns_query: "{hash}.checkdns.airvpn.org".into(),
    }
}
fn locals() -> Vec<IpAddr> {
    vec!["10.1.2.3".parse().unwrap(), "fd00::3".parse().unwrap()]
}
const TOKEN: &str = "0123456789abcdef0123456789abcdef";

#[derive(Clone, Copy)]
enum Answer {
    Correct,
    Wrong,
    Down,
}
/// Answers like the provider would, and records every request.
struct Fake {
    tunnel: Answer,
    dns: Answer,
    calls: RefCell<Vec<String>>,
}
impl Fake {
    fn new(tunnel: Answer, dns: Answer) -> Self {
        Self {
            tunnel,
            dns,
            calls: RefCell::new(vec![]),
        }
    }
    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}
impl Probe for Fake {
    fn tunnel(&mut self, local: IpAddr) -> Result<serde_json::Value> {
        self.calls.borrow_mut().push(format!("tun {local}"));
        match self.tunnel {
            Answer::Correct => Ok(json!({"ip": local.to_string()})),
            Answer::Wrong => Ok(json!({"ip": "198.51.100.200"})),
            Answer::Down => Err(anyhow!("503")),
        }
    }
    fn resolve(&mut self, host: &str) {
        self.calls.borrow_mut().push(format!("resolve {host}"));
    }
    fn dns(&mut self, local: IpAddr) -> Result<serde_json::Value> {
        self.calls.borrow_mut().push(format!("dns {local}"));
        match self.dns {
            Answer::Correct => Ok(json!({"dns": TOKEN})),
            Answer::Wrong => Ok(json!({"dns": "someone-else"})),
            Answer::Down => Err(anyhow!("timeout")),
        }
    }
}
fn run(fake: &mut Fake) -> (Verdict, Results, usize) {
    let mut results = Results::default();
    let published = Cell::new(0);
    let verdict = verify_once(
        &check(),
        &locals(),
        fake,
        TOKEN,
        &mut results,
        &mut |_| published.set(published.get() + 1),
        &|| false,
    );
    (verdict, results, published.get())
}

#[test]
fn a_healthy_tunnel_is_verified_with_one_request_per_check() {
    let mut fake = Fake::new(Answer::Correct, Answer::Correct);
    let (verdict, results, published) = run(&mut fake);
    assert_eq!(verdict, Verdict::Verified);
    assert_eq!(
        [results.ipv4, results.ipv6, results.dns],
        [Outcome::Verified; 3]
    );
    assert!(published > 0);
    assert_eq!(
        fake.calls(),
        vec![
            "tun 10.1.2.3".to_owned(),
            "tun fd00::3".into(),
            format!("resolve {TOKEN}.checkdns.airvpn.org"),
            "dns 10.1.2.3".into(),
        ]
    );
}

#[test]
fn a_provider_observed_mismatch_fails_and_stops_the_connection() {
    let mut fake = Fake::new(Answer::Wrong, Answer::Correct);
    let (verdict, results, _) = run(&mut fake);
    assert!(matches!(verdict, Verdict::Failed(_)) && verdict.stops_connection());
    assert_eq!(results.ipv4, Outcome::Failed);
    assert_eq!(fake.calls().len(), 1, "no further requests after a failure");
    assert!(results.failure().is_some());

    let mut fake = Fake::new(Answer::Correct, Answer::Wrong);
    let (verdict, results, _) = run(&mut fake);
    assert!(verdict.stops_connection());
    assert_eq!(results.dns, Outcome::Failed);
}

#[test]
fn an_unavailable_check_service_never_disconnects_and_is_not_hammered() {
    let mut fake = Fake::new(Answer::Down, Answer::Correct);
    let (verdict, results, _) = run(&mut fake);
    assert_eq!(verdict, Verdict::Unavailable);
    assert!(!verdict.stops_connection());
    assert_eq!(fake.calls(), vec!["tun 10.1.2.3".to_owned()]);
    assert_eq!(results.ipv4, Outcome::Unavailable);
    assert!(results.issue().is_some(), "shown as not verified");
    assert!(results.failure().is_none(), "never blamed for a disconnect");

    let mut fake = Fake::new(Answer::Correct, Answer::Down);
    let (verdict, results, _) = run(&mut fake);
    assert_eq!(verdict, Verdict::Unavailable);
    assert_eq!(results.dns, Outcome::Unavailable);
    assert!(results.failure().is_none());
}

#[test]
fn unpinned_check_services_are_refused_without_any_request() {
    let mut fake = Fake::new(Answer::Correct, Answer::Correct);
    let mut bad = check();
    bad.domain = "attacker.example".into();
    let verdict = verify_once(
        &bad,
        &locals(),
        &mut fake,
        TOKEN,
        &mut Results::default(),
        &mut |_| (),
        &|| false,
    );
    assert_eq!(verdict, Verdict::Unavailable);
    assert!(fake.calls().is_empty());
}

#[test]
fn stopping_interrupts_verification() {
    let mut fake = Fake::new(Answer::Correct, Answer::Correct);
    let verdict = verify_once(
        &check(),
        &locals(),
        &mut fake,
        TOKEN,
        &mut Results::default(),
        &mut |_| (),
        &|| true,
    );
    assert_eq!(verdict, Verdict::Stopped);
    assert!(fake.calls().is_empty());
}

#[test]
fn connect_time_retries_are_few_spaced_and_bounded() {
    let mut passes = 0;
    let mut pauses = vec![];
    // Even a huge retry option is capped.
    let verdict = verify_at_connect(
        3600,
        &mut || {
            passes += 1;
            Verdict::Unavailable
        },
        &mut |d| pauses.push(d),
        &|| false,
    );
    assert_eq!(verdict, Verdict::Unavailable);
    assert_eq!(passes, MAX_ATTEMPTS);
    assert_eq!(pauses, vec![Duration::from_secs(1), Duration::from_secs(2)]);
    assert_eq!(retry_pause(9), Duration::from_secs(4));

    let mut passes = 0;
    let verdict = verify_at_connect(
        3,
        &mut || {
            passes += 1;
            if passes == 2 {
                Verdict::Verified
            } else {
                Verdict::Unavailable
            }
        },
        &mut |_| (),
        &|| false,
    );
    assert_eq!((verdict, passes), (Verdict::Verified, 2));

    let verdict = verify_at_connect(0, &mut || Verdict::Skipped, &mut |_| (), &|| false);
    assert_eq!(verdict, Verdict::Skipped, "zero retries still runs once");
    let verdict = verify_at_connect(3, &mut || unreachable!(), &mut |_| (), &|| true);
    assert_eq!(verdict, Verdict::Stopped);
}

/// Provider checks run once per session, right after the first handshake.
/// Only the local route check repeats; it sends nothing to AirVPN.
#[test]
fn a_day_long_session_sends_provider_checks_exactly_once() {
    let mut schedule = Schedule::default();
    let mut verify = 0;
    let mut routes = 0;
    for second in 0..(24 * 3600u64) {
        let handshake = second >= 3;
        match schedule.tick(Duration::from_secs(second), handshake) {
            Due::Verify => {
                assert_eq!(second, 3);
                verify += 1;
            }
            Due::Routes => routes += 1,
            Due::Idle => (),
        }
    }
    assert_eq!(verify, 1);
    assert_eq!(routes as u64, (24 * 3600 - 3) / ROUTE_INTERVAL.as_secs());
    let mut idle = Schedule::default();
    assert_eq!(idle.tick(Duration::from_secs(600), false), Due::Idle);
}

/// A 10-minute check-service outage across a fleet: connected clients send
/// nothing and stay up; clients connecting during it send one request each
/// and stay up unverified. Nobody disconnects, so nobody reconnects in a wave.
#[test]
fn a_check_service_outage_does_not_disconnect_the_fleet() {
    let outage = Duration::from_secs(3600)..Duration::from_secs(4200);
    let mut requests_during_outage = 0;
    let mut joined_during_outage = 0;
    let mut disconnects = 0;
    for client in 0..1000u64 {
        let mut schedule = Schedule::default();
        let joined = Duration::from_secs(client * 7 % 7200);
        joined_during_outage += usize::from(outage.contains(&joined));
        for second in 0..7200u64 {
            let now = Duration::from_secs(second);
            if now < joined {
                continue;
            }
            if schedule.tick(now - joined, true) == Due::Verify {
                let down = outage.contains(&now);
                let mut fake = Fake::new(
                    if down { Answer::Down } else { Answer::Correct },
                    Answer::Correct,
                );
                let (verdict, _, _) = run(&mut fake);
                if down {
                    requests_during_outage += fake.calls().len();
                }
                disconnects += usize::from(verdict.stops_connection());
            }
        }
    }
    assert!(joined_during_outage > 0);
    assert_eq!(requests_during_outage, joined_during_outage);
    assert_eq!(disconnects, 0);
}

#[test]
fn reconnect_backoff_is_jittered_within_twenty_percent() {
    for attempt in 0..10 {
        let base = retry_delay(attempt);
        let low = retry_delay_jittered(attempt, 0.);
        let high = retry_delay_jittered(attempt, 1.);
        assert_eq!(low, base.mul_f64(0.8));
        assert_eq!(high, base.mul_f64(1.2));
        assert!(high <= Duration::from_secs(144));
    }
    assert_eq!(
        jitter(Duration::from_secs(10), f64::NAN),
        Duration::from_secs(10)
    );
    assert_eq!(jitter(Duration::from_secs(10), 7.), Duration::from_secs(12));
}

#[test]
fn refresh_interval_respects_the_floor_and_manifest_hint() {
    assert_eq!(interval(15, None), MIN_AUTOMATIC);
    assert_eq!(interval(15, Some(30)), Duration::from_secs(1800));
    assert_eq!(interval(7200, Some(30)), Duration::from_secs(7200));
    assert_eq!(interval(600, Some(60)), Duration::from_secs(3600));
    assert_eq!(failure_delay(1, Duration::from_secs(1800)), FAILURE_BASE);
    assert_eq!(
        failure_delay(2, Duration::from_secs(1800)),
        FAILURE_BASE * 2
    );
    assert_eq!(failure_delay(30, Duration::from_secs(1800)), FAILURE_CAP);
    assert_eq!(
        failure_delay(30, Duration::from_secs(4 * 3600)),
        Duration::from_secs(4 * 3600)
    );
}

#[test]
fn manual_refresh_is_rate_limited_and_single_flight() {
    let t0 = Instant::now();
    let mut s = RefreshSchedule::new(t0);
    assert!(s.manual_allowed(t0));
    assert!(!s.due(t0), "sign-in has just fetched everything");
    s.start(t0);
    assert!(
        !s.manual_allowed(t0 + MIN_MANUAL * 5),
        "one request at a time"
    );
    // Unit 0: the earliest jittered deadline, exactly the interval (R2).
    s.succeeded(t0 + Duration::from_secs(2), Duration::from_secs(1800), 0.0);
    assert!(!s.manual_allowed(t0 + Duration::from_secs(30)));
    assert!(s.manual_allowed(t0 + Duration::from_secs(2) + MIN_MANUAL));
    assert!(!s.due(t0 + Duration::from_secs(1801)));
    assert!(s.due(t0 + Duration::from_secs(1802)));
}

/// Six hours of unreachable bootstrap servers: few attempts, growing gaps,
/// jitter spreads clients, and the first success resets the backoff.
#[test]
fn refresh_failures_back_off_and_recover() {
    let t0 = Instant::now();
    let period = Duration::from_secs(1800);
    let mut s = RefreshSchedule::new(t0);
    let mut attempts = vec![];
    for second in 0..(6 * 3600u64) {
        let now = t0 + Duration::from_secs(second);
        if s.due(now) {
            attempts.push(second);
            s.start(now);
            s.failed(now, period, (second % 7) as f64 / 7.);
        }
    }
    assert!(attempts.len() <= 9, "{attempts:?}");
    let gaps: Vec<_> = attempts.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(gaps.iter().all(|g| *g >= 5 * 60), "{gaps:?}");
    assert!(gaps.iter().all(|g| *g <= 72 * 60), "{gaps:?}");
    assert!(s.failures() >= 5);
    let now = t0 + Duration::from_secs(6 * 3600);
    s.start(now);
    s.succeeded(now, period, 0.);
    assert_eq!(s.failures(), 0);
    // Jitter never schedules before next_update (R2); unit 0 is exactly it.
    assert_eq!(s.next(), now + period);
}
