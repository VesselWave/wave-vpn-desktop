// SPDX-License-Identifier: GPL-3.0-or-later
// Included only by the GUI unit-test target: real `Wave::update`/`transport_line`
// wiring for the R1-R4 fixes and probe sampling. Nothing here starts a thread
// that touches the network or the privileged helper.
#![cfg(test)]
use super::*;

const KEY: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";

fn account(devices: &[&str]) -> Account {
    let keys: String = devices
        .iter()
        .map(|name| format!(r#"<key name="{name}" wg_private_key="{KEY}" wg_preshared="{KEY}" wg_ipv4="10.1.2.3/32" wg_dns_ipv4="10.1.0.1"/>"#))
        .collect();
    wave_protocol::parse_account(
        &format!(r#"<user login="synthetic" wg_public_key="{KEY}"><keys>{keys}</keys></user>"#),
        "synthetic",
    )
    .unwrap()
}
fn manifest() -> Manifest {
    let mut m = wave_protocol::parse_manifest(include_str!(
        "../../protocol-data/fixtures/eddie-manifest.xml"
    ))
    .unwrap();
    for mode in &mut m.modes {
        mode.entry_index = 0;
    }
    for server in &mut m.servers {
        server.entries = vec!["192.0.2.10".parse().unwrap()];
    }
    m
}
/// Signed in, preview (no networking), bootstrap store in a temporary directory.
fn app(dir: &tempfile::TempDir) -> Wave {
    let mut app = Wave::new();
    app.preview = true;
    app.prefs = Preferences::default();
    app.prefs.options = Options::default();
    app.prefs.options.set("ipv6", "in-block".into());
    app.bootstrap = BootstrapStore::in_directory(dir.path());
    app.account = Some(account(&["laptop"]));
    app.prefs.device = "laptop".into();
    app.manifest = Some(manifest());
    app.window = Some(iced::window::Id::unique()); // Tick runs to completion
    app
}
fn rotate(app: &Wave, url: &str, issued: u64) {
    let mut latest = wave_protocol::Bootstrap::eddie();
    latest.urls = vec![url.into()];
    app.bootstrap
        .save_fenced(&latest, issued, &Default::default(), &|| true)
        .unwrap();
}
fn refreshed(app: &mut Wave, manual: bool, account: Option<Account>) {
    app.refresh.start(Instant::now());
    app.sender
        .send(Event::Refreshed(
            app.account_epoch,
            manual,
            Box::new(Ok((account, manifest()))),
        ))
        .unwrap();
    let _ = app.update(Message::Tick);
}

// ---- R1

#[test]
fn fix_observed_mismatch_stays_failed_and_is_not_reconnected() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    app.prefs.options.set("reconnect", "true".into());
    app.status = "Connecting…".into();
    app.transport_line(
        r#"{"event":"checks","results":{"ipv4":"verified","ipv6":"unknown","dns":"failed"}}"#
            .into(),
    );
    // Later reports in the same session cannot clear it.
    app.transport_line(
        r#"{"event":"checks","results":{"ipv4":"verified","ipv6":"unknown","dns":"unavailable"}}"#
            .into(),
    );
    app.transport_line(
        r#"{"event":"checks","results":{"ipv4":"verified","ipv6":"unknown","dns":"verified"}}"#
            .into(),
    );
    assert_eq!(app.checks.dns, wave_protocol::checks::Outcome::Failed);
    app.transport_line(r#"{"event":"failure","retryable":false,"category":"verification"}"#.into());
    app.sender
        .send(Event::End("System DNS verification failed".into()))
        .unwrap();
    let _ = app.update(Message::Tick);
    assert!(app.retry_at.is_none(), "no automatic reconnect into a leak");
    assert_eq!(app.status, "Disconnected");
    assert!(app
        .error
        .as_deref()
        .unwrap()
        .contains("DNS verification failed"));
}

// ---- R3

#[test]
fn fix_bootstrap_rotation_updates_an_active_lock_through_the_helper() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    app.test_helper = Some(true);
    app.guarded = true;
    rotate(&app, "http://203.0.113.77:8080", 100);
    refreshed(&mut app, false, None);
    let (action, input) = app.test_privileged.last().cloned().unwrap();
    assert_eq!(action, "lock-update");
    assert_eq!(
        input.bootstrap,
        vec![Exemption {
            ip: "203.0.113.77".parse().unwrap(),
            port: 8080
        }]
    );
    // What the helper receives is exactly what it accepts.
    let bytes = serde_json::to_vec(&input).unwrap();
    assert_eq!(
        wave_protocol::transport::LockInput::parse(&bytes).unwrap(),
        input
    );
    // Unchanged bootstrap: no further helper call.
    app.network_busy = false;
    refreshed(&mut app, false, None);
    assert_eq!(app.test_privileged.len(), 1);
    // Rotation while connected waits for the connection to end.
    rotate(&app, "http://[2001:db8::77]:8443", 200);
    app.control = Some(Arc::new(Mutex::new(WorkerControl::default())));
    app.since = Some(Instant::now());
    refreshed(&mut app, false, None);
    assert_eq!(app.test_privileged.len(), 1);
    assert!(app.exemptions_pending);
    app.control = None;
    app.since = None;
    let _ = app.update(Message::Tick);
    let (action, input) = app.test_privileged.last().cloned().unwrap();
    assert_eq!((action, input.bootstrap[0].port), ("lock-update", 8443));
    assert!(!app.exemptions_pending);
}

#[test]
fn fix_rotation_without_a_lock_or_helper_never_calls_or_prompts() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    app.test_helper = Some(true);
    rotate(&app, "http://203.0.113.77:8080", 100);
    refreshed(&mut app, false, None);
    assert!(app.test_privileged.is_empty(), "no lock: nothing to update");
    app.guarded = true;
    app.test_helper = Some(false);
    refreshed(&mut app, false, None);
    assert!(
        app.test_privileged.is_empty(),
        "helper not installed: no prompt"
    );
}

#[test]
fn fix_lock_actions_send_the_latest_exemptions() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    app.preview = false; // the test hook replaces the helper process
    app.test_helper = Some(true);
    rotate(&app, "http://203.0.113.77:8080", 100);
    let _ = app.update(Message::LockAction("lock-enable"));
    let (action, input) = app.test_privileged.last().cloned().unwrap();
    assert_eq!(action, "lock-enable");
    assert_eq!(input.bootstrap.len(), 1);
    assert_eq!(
        app.applied_exemptions.as_deref(),
        Some(&input.bootstrap[..])
    );
}

#[test]
fn fix_lock_restored_at_boot_is_synced_once_status_confirms_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    app.test_helper = Some(true);
    rotate(&app, "http://203.0.113.77:8080", 100);
    app.sender
        .send(Event::NetworkStatus(
            app.network_revision,
            Ok(wave_protocol::protection::Status {
                enabled: Some(true),
                persistent: true,
                ..Default::default()
            }),
        ))
        .unwrap();
    let _ = app.update(Message::Tick);
    assert_eq!(app.test_privileged.len(), 1);
    assert_eq!(app.test_privileged[0].0, "lock-update");
}

// ---- R4

#[test]
fn fix_sign_out_fences_bootstrap_writes_of_the_old_session() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    let old = app.requests_cancel.clone();
    app.sender
        .send(Event::SignedOut(app.account_epoch, Ok(())))
        .unwrap();
    let _ = app.update(Message::Tick);
    assert!(old.load(Ordering::SeqCst));
    let mut late = wave_protocol::Bootstrap::eddie();
    late.urls = vec!["http://203.0.113.99".into()];
    let error = app
        .bootstrap
        .save_fenced(&late, 1, &Default::default(), &|| {
            !old.load(Ordering::SeqCst)
        })
        .unwrap_err();
    assert!(error.is::<wave_protocol::Cancelled>());
    assert_eq!(app.bootstrap.load(), None);
    assert!(app.applied_exemptions.is_none());
}

#[test]
fn fix_sign_out_forgets_latency_results_but_keeps_the_probe_cooldown() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    app.latency_store = Some(LatencyStore::in_directory(dir.path()));
    let now = Instant::now();
    let run = app.latency.begin(now);
    app.latency.finish(
        run,
        now,
        latency::Outcome::Complete([("Achernar".into(), latency::Ping::Measured(9))].into()),
    );
    app.sender
        .send(Event::SignedOut(app.account_epoch, Ok(())))
        .unwrap();
    let _ = app.update(Message::Tick);
    assert_eq!(app.latency.get("Achernar"), latency::Ping::Unknown);
    assert!(!app.latency.due(now + Duration::from_secs(60), false));
    let mut restarted = LatencyCache::default();
    LatencyStore::in_directory(dir.path()).load(&mut restarted, Instant::now());
    assert_eq!(
        restarted.get("Achernar"),
        latency::Ping::Unknown,
        "not persisted"
    );
    assert!(!restarted.due(Instant::now(), false), "cooldown persisted");
}

// ---- Account refresh

#[test]
fn fix_manual_refresh_recovers_a_removed_device_without_sign_out() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    app.profile().unwrap();
    refreshed(&mut app, true, Some(account(&["desktop", "phone"])));
    assert_eq!(app.prefs.device, "desktop");
    assert!(
        app.profile().is_ok(),
        "Connect works with the new device list"
    );
    // An automatic refresh carries no account and keeps the current one.
    refreshed(&mut app, false, None);
    assert_eq!(app.account.as_ref().unwrap().devices.len(), 2);
    assert!(!schedule::account_due(app.account_checked, Instant::now()));
}

#[test]
fn fix_automatic_account_refusal_is_shown() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    app.refresh.start(Instant::now());
    app.sender
        .send(Event::Refreshed(
            app.account_epoch,
            false,
            Box::new(Err("AirVPN rejected the account request.".into())),
        ))
        .unwrap();
    let _ = app.update(Message::Tick);
    assert!(app.error.as_deref().unwrap().contains("rejected"));
    assert!(app.account.is_some() && app.manifest.is_some());
}

// ---- Probe load

#[test]
fn fix_probe_sample_is_bounded_and_includes_the_explicit_server() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    let fleet = app.manifest.as_ref().unwrap().servers.len();
    let explicit = app.manifest.as_ref().unwrap().servers[fleet - 1]
        .name
        .clone();
    app.prefs.target = Target::Server(explicit.clone());
    let targets = app.latency_targets(7).unwrap();
    assert!(targets.len() <= latency::SAMPLE_BUDGET && targets.len() < fleet);
    assert_eq!(targets[0].0, explicit);
    // Explicit selection still connects whatever the latency data says.
    assert_eq!(app.selected().unwrap().name, explicit);
    app.profile().unwrap();
}

#[test]
fn fix_no_probes_while_connecting_connected_or_changing_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    let now = Instant::now();
    assert!(app.latency.due(now, app.probes_blocked()));
    app.network_busy = true;
    assert!(!app.latency.due(now, app.probes_blocked()));
    app.network_busy = false;
    app.control = Some(Arc::new(Mutex::new(WorkerControl::default())));
    assert!(!app.latency.due(now, app.probes_blocked()), "connecting");
    app.since = Some(now);
    assert!(!app.latency.due(now, app.probes_blocked()), "connected");
}
