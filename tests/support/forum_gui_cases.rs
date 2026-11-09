// SPDX-License-Identifier: GPL-3.0-or-later
// Included only by the GUI unit-test target. Forum/Android-parity contracts
// (docs/qa/ANDROID-0.1.5-PARITY.md, docs/qa/DESKTOP-BUG-INTAKE.md) driven
// through the real `Wave::update`/`transport_line` wiring. Nothing here
// starts a networking thread or the privileged helper.
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

/// Signed in with cached data, preview on (no thread touches the network).
fn app() -> Wave {
    let mut app = Wave::new();
    app.preview = true;
    app.prefs = Preferences::default();
    app.prefs.options = Options::default();
    app.prefs.options.set("ipv6", "in-block".into());
    app.account = Some(account(&["test"]));
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
    app.manifest = Some(m);
    app.prefs.device = "test".into();
    app
}

fn end(app: &mut Wave, message: &str) {
    app.sender.send(Event::End(message.into())).unwrap();
    let _ = app.update(Message::Tick);
}

fn running(app: &mut Wave, server: &str) {
    app.control = Some(Arc::new(Mutex::new(WorkerControl::default())));
    app.active_server = app
        .manifest
        .as_ref()
        .unwrap()
        .servers
        .iter()
        .find(|s| s.name == server)
        .cloned();
    assert!(app.active_server.is_some());
}

fn penalized(app: &Wave, server: &str) -> bool {
    app.penalties
        .get(server)
        .is_some_and(|until| *until > Instant::now())
}

// P-02 / D-05: endpoint backoff must not penalize user cancellation.
#[test]
fn forum_d05_user_disconnect_does_not_penalize_the_server() {
    let mut app = app();
    let best = app.selected().unwrap().name.clone();
    running(&mut app, &best);
    app.since = Some(Instant::now());
    app.status = "Connected".into();
    let _ = app.update(Message::Disconnect);
    end(&mut app, "");
    assert!(
        !penalized(&app, &best),
        "a clean user Disconnect is not a server failure"
    );
    assert_eq!(app.selected().unwrap().name, best, "Best stays the same");
}

#[test]
fn forum_d05_cancelled_connect_does_not_penalize_the_server() {
    let mut app = app();
    let best = app.selected().unwrap().name.clone();
    running(&mut app, &best);
    let _ = app.update(Message::Disconnect); // Cancel while Connecting.
    end(
        &mut app,
        "Networking service setup cancelled or timed out; retry Connect when ready",
    );
    assert!(!penalized(&app, &best));
}

#[test]
fn forum_d05_failed_connection_still_penalizes_and_fails_over() {
    let mut app = app();
    let best = app.selected().unwrap().name.clone();
    running(&mut app, &best);
    app.retryable = true;
    end(&mut app, "First WireGuard handshake timed out");
    assert!(penalized(&app, &best));
    assert_ne!(app.selected().unwrap().name, best, "retry fails over");
    assert!(app.retry_at.is_some());
}

// P-01: a failed refresh never gates Connect; cached keys stay usable.
#[test]
fn forum_p01_refresh_failure_or_deadline_does_not_gate_cached_connect() {
    let mut app = app();
    app.refresh.start(Instant::now());
    assert!(
        app.profile().is_ok(),
        "connect does not wait for a running refresh"
    );
    app.sender
        .send(Event::Refreshed(
            app.account_epoch,
            false,
            Box::new(Err("Unable to refresh AirVPN data".into())),
        ))
        .unwrap();
    let _ = app.update(Message::Tick);
    assert!(
        app.profile().is_ok(),
        "cached account and servers still connect"
    );
    assert!(!app.refresh.running());
}

// P-04 / D-03: retained traffic blocking is visible and offers the way out.
#[test]
fn forum_d03_retained_network_lock_says_traffic_is_blocked() {
    let mut app = app();
    app.transport_line(r#"{"event":"protection","enabled":true}"#.into());
    end(&mut app, "First WireGuard handshake timed out");
    app.retry_at = None;
    app.protection_known = true;
    app.guarded = true;
    assert!(app.blocking_traffic());
    assert_eq!(app.display_status(), "Blocking traffic");
    assert!(
        app.protection_note().is_some_and(
            |note| note.contains("no internet") && note.contains("Turn off Network Lock")
        )
    );
    assert_eq!(app.tray_snapshot().status, "Blocking traffic");
    // Unconfirmed protection is never reported as blocking.
    app.protection_known = false;
    assert!(!app.blocking_traffic());
    assert_ne!(app.display_status(), "Blocking traffic");
}

#[test]
fn forum_d03_turn_off_network_lock_from_overview_is_an_explicit_disable() {
    let mut app = app();
    app.preview = false; // LockAction is disabled in preview; the helper is hooked.
    app.test_helper = Some(true);
    app.protection_known = true;
    app.guarded = true;
    let _ = app.update(Message::LockAction("lock-disable"));
    assert_eq!(app.test_privileged.len(), 1);
    assert_eq!(app.test_privileged[0].0, "lock-disable");
    assert_eq!(app.lock_change, Some(false));
}

// P-07 / D-04: Cancel stays reachable, including from the tray during the
// reconnect wait.
#[test]
fn forum_d04_cancel_is_reachable_while_connecting_and_waiting_to_retry() {
    let mut app = app();
    app.control = Some(Arc::new(Mutex::new(WorkerControl::default())));
    assert!(app.power_cancels());
    assert!(app.tray_snapshot().active);
    app.control = None;
    app.retry_at = Some(Instant::now() + Duration::from_secs(30));
    app.status = "Reconnecting…".into();
    assert!(app.power_cancels());
    assert!(
        app.tray_snapshot().active,
        "the tray must offer Disconnect (cancel), not an immediate Connect"
    );
    app.sender
        .send(Event::Tray(tray::Action::Disconnect))
        .unwrap();
    let _ = app.update(Message::Tick);
    assert!(app.retry_at.is_none());
    assert!(!app.tray_snapshot().active);
}

// P-03 / D-06: helper missing, refused or outdated is explained.
#[test]
fn forum_d06_missing_helper_is_explained_instead_of_unknown_status() {
    let mut app = app();
    app.test_helper = Some(false);
    app.protection_known = false;
    let note = app.protection_note().unwrap();
    assert!(note.contains("networking helper isn't installed"), "{note}");
    app.test_helper = Some(true);
    assert!(app
        .protection_note()
        .is_some_and(|n| n.contains("Protection status is unavailable")));
}

#[test]
fn forum_d06_helper_refusal_explains_the_failed_connection() {
    for (line, expected) in [
        (
            "This user is not authorized for Wave VPN networking",
            "different desktop user",
        ),
        (
            "Error: unknown field `bootstrap`, expected one of `profile`, `options`",
            "older than this app",
        ),
        (
            "Networking service rejected or could not complete this request",
            "older than this app",
        ),
    ] {
        let mut app = app();
        running(&mut app, "Achernar");
        app.transport_line(line.into());
        end(
            &mut app,
            "Connection stopped unexpectedly; inspect Logs before retrying",
        );
        let error = app.error.clone().unwrap();
        assert!(error.contains(expected), "{line} -> {error}");
        assert!(error.contains("sudo wave-vpn service-install"), "{error}");
        assert!(app.retry_at.is_none(), "a refusal is not retried");
    }
}

#[test]
fn forum_d06_declined_administrator_prompt_is_explained() {
    let mut app = app();
    running(&mut app, "Achernar");
    end(&mut app, "Networking service setup was cancelled or failed");
    let error = app.error.clone().unwrap();
    assert!(error.contains("administrator"), "{error}");
    assert!(error.contains("sudo wave-vpn service-install"), "{error}");
    // A refusal from one session does not leak into the next.
    running(&mut app, "Achernar");
    end(&mut app, "First WireGuard handshake timed out");
    assert_eq!(
        app.error.as_deref(),
        Some("First WireGuard handshake timed out")
    );
}

// P-05 / D-07: a target matching no server is refused with a named reason,
// never widened to another server or country.
#[test]
fn forum_d07_target_matching_no_server_is_refused_with_its_name() {
    let mut app = app();
    app.preview = false; // Connect is a no-op in preview; this path starts no thread.
    app.test_helper = Some(true);
    app.prefs.target = Target::Country("zz".into());
    let _ = app.update(Message::Connect);
    assert!(app.control.is_none(), "nothing starts");
    let error = app.error.clone().unwrap();
    assert!(
        error.contains("ZZ") && error.contains("Best server"),
        "{error}"
    );
    app.prefs.target = Target::Server("Nonexistent".into());
    let _ = app.update(Message::Connect);
    assert!(app.control.is_none());
    assert!(app.error.as_deref().unwrap().contains("Nonexistent"));
}

// P-09: the device setting is hidden for single-device accounts.
#[test]
fn forum_p09_device_choice_is_hidden_for_a_single_device_account() {
    let mut app = app();
    assert!(app.device_choices().is_none());
    app.account = Some(account(&["laptop", "phone"]));
    assert_eq!(
        app.device_choices().unwrap(),
        vec!["laptop".to_owned(), "phone".to_owned()]
    );
}

// P-08: state and secondary text meet WCAG AA (4.5:1) on every surface in
// both themes, matching Android 0.1.4's light tokens.
fn luminance(c: iced::Color) -> f32 {
    let channel = |v: f32| {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}
fn contrast(a: iced::Color, b: iced::Color) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}
#[test]
fn forum_p08_state_and_secondary_text_meet_contrast_in_both_themes() {
    use theme::Ink;
    for dark in [true, false] {
        let t = theme::palette(dark);
        let c = theme::colors(&t);
        for (name, role) in [
            ("muted", Ink::Muted),
            ("accent", Ink::Accent),
            ("warning", Ink::Warning),
            ("danger", Ink::Danger),
        ] {
            for (surface, background) in
                [("bg", c.bg), ("surface", c.surface), ("sidebar", c.sidebar)]
            {
                let ratio = contrast(theme::color(&t, role), background);
                assert!(
                    ratio >= 4.5,
                    "{name} on {surface} ({}) is {ratio:.2}:1",
                    if dark { "dark" } else { "light" }
                );
            }
        }
    }
}
