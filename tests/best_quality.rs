// SPDX-License-Identifier: GPL-3.0-or-later
//! Invented representative access-network RTTs, not measurements or performance promises.
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use wave_protocol::{
    latency::{LatencyCache, LatencyStore, Ping, MAX_AGE},
    parse_manifest,
};
#[derive(serde::Deserialize)]
struct Case {
    name: String,
    pings: BTreeMap<String, Option<i32>>,
    loads: BTreeMap<String, u64>,
    users: BTreeMap<String, u32>,
    expected: String,
}
#[test]
fn representative_network_choices() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/best-networks.json")).unwrap();
    for c in cases {
        let mut m =
            parse_manifest(include_str!("../protocol-data/fixtures/eddie-manifest.xml")).unwrap();
        let base = m.servers[0].clone();
        m.servers = c
            .loads
            .iter()
            .map(|(name, load)| {
                let mut s = base.clone();
                s.name = name.clone();
                s.country = name.to_lowercase();
                s.location = name.clone();
                s.capacity = 1000;
                s.bandwidth = load * 625_000;
                s.users = c.users[name];
                s.users_max = 100;
                s.score_base = 0.;
                s.warning_open.clear();
                s.warning_closed.clear();
                s
            })
            .collect();
        let mut cache = LatencyCache::default();
        cache.set_locations(&m.servers);
        for (n, p) in &c.pings {
            if let Some(p) = p {
                cache.insert(
                    n.clone(),
                    if *p < 0 {
                        Ping::Unreachable
                    } else {
                        Ping::Measured(*p as u32)
                    },
                );
            }
        }
        let selected = m
            .servers
            .iter()
            .min_by_key(|s| {
                (
                    m.desktop_score(s, cache.ranking(&s.name), 0).unwrap(),
                    s.name.clone(),
                )
            })
            .unwrap();
        let fastest = c
            .pings
            .values()
            .flatten()
            .filter(|p| **p > 0)
            .min()
            .copied();
        let regret = c.pings[&selected.name]
            .filter(|p| *p > 0)
            .zip(fastest)
            .map(|(a, b)| a - b);
        println!("{} Linux={} RTT-regret={regret:?}", c.name, selected.name);
        assert_eq!(selected.name, c.expected, "{}", c.name);
    }
}
#[test]
fn expired_in_memory_samples_do_not_survive_failed_refresh() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("latency.json"),
        r#"{"version":1,"next":0,"measured":1000,"pings":{"old":{"ms":1,"at":1000}}}"#,
    )
    .unwrap();
    let mut c = LatencyCache::default();
    LatencyStore::in_directory(d.path()).load_at(
        &mut c,
        Instant::now() - MAX_AGE - Duration::from_secs(1),
        1000,
    );
    let mut m =
        parse_manifest(include_str!("../protocol-data/fixtures/eddie-manifest.xml")).unwrap();
    let mut old = m.servers.remove(0);
    old.name = "old".into();
    let mut alias = old.clone();
    alias.name = "alias".into();
    c.set_locations(&[old, alias]);
    assert_eq!(c.get("old"), Ping::Unknown);
    assert_eq!(c.get("alias"), Ping::Unknown);
    assert_eq!(c.ranking("new"), Ping::Unknown);
    c.insert("fresh".into(), Ping::Measured(80));
    assert_eq!(c.ranking("new"), Ping::Measured(80));
}

#[test]
fn bounded_coverage_cannot_guarantee_the_nearby_location() {
    let m = parse_manifest(include_str!("../protocol-data/fixtures/eddie-manifest.xml")).unwrap();
    let fleet: Vec<_> = (0..40)
        .map(|i| {
            let mut s = m.servers[0].clone();
            s.name = format!("S{i}");
            s.location = format!("City{i}");
            s.entries = vec!["8.8.8.8".parse().unwrap()];
            s
        })
        .collect();
    let cache = LatencyCache::default();
    let sample = cache.sample(&fleet, 32, &[], 0);
    assert_eq!(sample.len(), 32);
    let omitted = fleet
        .iter()
        .find(|s| !sample.iter().any(|(n, _)| *n == s.name))
        .unwrap();
    // No proximity signal in the sampler: this omitted city could be the user's own.
    let priority = cache.sample(&fleet, 32, &[&omitted.name], 0);
    assert!(priority.iter().any(|(n, _)| *n == omitted.name));
}

#[test]
fn recorded_five_location_sample_selects_lowest_observed_rtt() {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/best-live-public.json")).unwrap();
    let m = parse_manifest(include_str!("../protocol-data/fixtures/eddie-manifest.xml")).unwrap();
    let selected = rows
        .iter()
        .min_by_key(|r| {
            let mut s = m.servers[0].clone();
            s.capacity = r["capacity"].as_u64().unwrap();
            s.bandwidth = r["load"].as_f64().unwrap() as u64 * s.capacity * 625;
            s.users = r["users"].as_u64().unwrap() as u32;
            s.users_max = r["users_max"].as_u64().unwrap() as u32;
            s.score_base = r["base"].as_f64().unwrap();
            s.warning_open.clear();
            s.warning_closed.clear();
            let mut p: Vec<_> = r["tcp_ms"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as u32)
                .collect();
            p.sort();
            m.desktop_score(&s, Ping::Measured(p[1]), 0).unwrap()
        })
        .unwrap();
    assert_eq!(selected["name"], "Guniibuu");
}
