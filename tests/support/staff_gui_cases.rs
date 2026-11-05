// SPDX-License-Identifier: GPL-3.0-or-later
// Included only by the GUI unit-test target; also harmless as an integration target.
#![cfg(test)]
use super::*;

fn cached_app() -> Wave {
    let mut app = Wave::new();
    app.preview = true; // Every thread-launching network action is disabled.
    app.prefs = Preferences::default();
    app.prefs.options = Options::default();
    app.prefs.options.set("ipv6", "in-block".into());
    let key = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";
    app.account = Some(wave_protocol::parse_account(&format!(r#"<user login="synthetic" wg_public_key="{key}"><keys><key name="test" wg_private_key="{key}" wg_preshared="{key}" wg_ipv4="10.1.2.3/32" wg_dns_ipv4="10.1.0.1"/></keys></user>"#), "synthetic").unwrap());
    app.manifest = Some(
        wave_protocol::parse_manifest(include_str!(
            "../../protocol-data/fixtures/eddie-manifest.xml"
        ))
        .unwrap(),
    );
    for mode in &mut app.manifest.as_mut().unwrap().modes {
        mode.entry_index = 0;
    }
    for server in &mut app.manifest.as_mut().unwrap().servers {
        server.entries = vec!["192.0.2.10".parse().unwrap()];
    }
    app.prefs.device = "test".into();
    app
}

#[test]
fn staff_refresh_failure_keeps_cached_connect_profile_and_current_pings() {
    let mut app = cached_app();
    app.profile().unwrap();
    app.latency
        .insert("cached".into(), latency::Ping::Measured(23));
    app.refresh.start(Instant::now());
    app.sender
        .send(Event::Refreshed(
            app.account_epoch,
            true,
            Box::new(Err("offline".into())),
        ))
        .unwrap();
    let _ = app.update(Message::Tick);
    assert!(
        app.profile().is_ok(),
        "cached Connect remains usable after refresh failure"
    );
    assert_eq!(app.latency.get("cached"), latency::Ping::Measured(23));
    assert!(!app.refresh.running());
    assert!(app.error.as_ref().unwrap().contains(KEEPING_SERVERS));
    for _ in 0..100 {
        let _ = app.update(Message::Refresh);
    }
    assert_eq!(app.refresh.failures(), 1);
    assert!(!app.refresh.running());
}

#[test]
fn staff_stale_success_cannot_replace_new_session_or_unblock_its_request() {
    let mut app = cached_app();
    let count = app.manifest.as_ref().unwrap().servers.len();
    app.refresh.start(Instant::now());
    let mut stale = app.manifest.clone().unwrap();
    stale.servers.clear();
    app.sender
        .send(Event::Refreshed(
            app.account_epoch.wrapping_add(1),
            true,
            Box::new(Ok((None, stale))),
        ))
        .unwrap();
    let _ = app.update(Message::Tick);
    assert_eq!(app.manifest.as_ref().unwrap().servers.len(), count);
    assert!(app.refresh.running());
    app.profile().unwrap();
}

#[test]
fn staff_transport_events_keep_unavailable_distinct_from_verified() {
    let mut app = cached_app();
    app.transport_line(
        r#"{"event":"checks","results":{"ipv4":"verified","ipv6":"unavailable","dns":"unknown"}}"#
            .into(),
    );
    app.transport_line(r#"{"event":"connected"}"#.into());
    assert_eq!(app.status, "Connected");
    assert_eq!(app.checks.ipv6, wave_protocol::checks::Outcome::Unavailable);
    assert!(app.checks.issue().unwrap().contains("not verified"));
    assert!(app.checks.failure().is_none());
}

#[test]
fn staff_lock_and_retry_transitions_block_expired_latency_work() {
    let mut app = cached_app();
    let now = Instant::now();
    assert!(app.latency.due(now, app.probes_blocked()));
    app.transport_line(r#"{"event":"protection","enabled":true}"#.into());
    assert!(!app.latency.due(now, app.probes_blocked()));
    app.guarded = false;
    app.retry_at = Some(now + Duration::from_secs(30));
    assert!(!app.latency.due(now, app.probes_blocked()));
    app.retry_at = None;
    app.control = Some(Arc::new(Mutex::new(WorkerControl::default())));
    assert!(!app.latency.due(now, app.probes_blocked()));
}
