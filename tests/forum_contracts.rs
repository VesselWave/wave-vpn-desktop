// SPDX-License-Identifier: GPL-3.0-or-later
//! Forum/parity contracts for manifest handling (docs/qa/DESKTOP-BUG-INTAKE.md).
//! Offline: synthetic documents only. `scripts/check-forum.py` runs every
//! `forum_` test and reports the named contracts.
use wave_protocol::parse_manifest;

const MODULUS: &str = include_str!("../protocol-data/fixtures/eddie-manifest.xml");

/// The fixture's bootstrap attributes, so the synthetic manifest has a valid
/// optional bootstrap unless a test replaces it.
fn bootstrap_attributes() -> String {
    let start = MODULUS.find("auth_rsa_exponent=").unwrap();
    let end = MODULUS[start..].find('>').unwrap() + start;
    MODULUS[start..end].to_owned()
}

fn manifest(modes: &str, servers: &str) -> String {
    format!(
        r#"<manifest time="1757521167" {}>
        <urls><url address="http://192.0.2.1"/></urls>
        <modes>{modes}</modes>
        <servers_groups><servers_group group="g" bw_max="1000" country_code="nl" location="City"/></servers_groups>
        <servers>{servers}</servers>
        </manifest>"#,
        bootstrap_attributes()
    )
}

const MODE: &str = r#"<mode type="wireguard" port="1637" entry_index="0"/>"#;

#[test]
fn forum_d10_one_malformed_server_address_keeps_the_other_servers() {
    // Eddie's IpAddresses.Add skips an invalid entry instead of failing the list.
    let m = parse_manifest(&manifest(
        MODE,
        r#"<server name="Good" group="g" ips_entry="192.0.2.10" ips_exit="192.0.2.11"/>
           <server name="Odd" group="g" ips_entry="192.0.2.20,not-an-address,999.1.1.1" ips_exit="bad"/>"#,
    ))
    .expect("a malformed address in one server must not reject the server list");
    assert_eq!(m.servers.len(), 2);
    let odd = m.servers.iter().find(|s| s.name == "Odd").unwrap();
    assert_eq!(
        odd.entries,
        vec!["192.0.2.20".parse::<std::net::IpAddr>().unwrap()]
    );
    assert!(odd.exits.is_empty());
    assert!(odd.available());
}

#[test]
fn forum_d10_server_left_without_entries_refuses_connect_clearly() {
    let m = parse_manifest(&manifest(
        MODE,
        r#"<server name="Good" group="g" ips_entry="192.0.2.10"/>
           <server name="Empty" group="g" ips_entry="garbage"/>"#,
    ))
    .unwrap();
    let empty = m.servers.iter().find(|s| s.name == "Empty").unwrap();
    assert!(empty.entries.is_empty());
    let error = empty.endpoint(0, false).unwrap_err().to_string();
    assert!(error.contains("no matching entry address"), "{error}");
}

#[test]
fn forum_d11_manifest_without_servers_is_rejected_so_the_cache_survives() {
    let error = parse_manifest(&manifest(MODE, "")).err();
    assert!(
        error.is_some(),
        "an empty server list must fail; the caller keeps its cached servers"
    );
}

#[test]
fn forum_d11_manifest_without_wireguard_modes_is_rejected() {
    let error = parse_manifest(&manifest(
        r#"<mode type="openvpn" port="443" entry_index="0"/>"#,
        r#"<server name="Good" group="g" ips_entry="192.0.2.10"/>"#,
    ))
    .err();
    assert!(error.is_some());
}

#[test]
fn forum_d11_malformed_wireguard_mode_is_skipped_not_fatal() {
    let m = parse_manifest(&manifest(
        r#"<mode type="wireguard" port="nope" entry_index="0"/>
           <mode type="wireguard" port="51820" entry_index="99"/>
           <mode type="wireguard" port="1637" entry_index="0"/>"#,
        r#"<server name="Good" group="g" ips_entry="192.0.2.10"/>"#,
    ))
    .expect("one malformed mode must not discard the server list");
    assert_eq!(m.modes.len(), 1);
    assert_eq!(m.modes[0].port, 1637);
}

#[test]
fn forum_p02_invalid_optional_bootstrap_keeps_valid_servers() {
    // Parity with Android 0.1.5 / staff comment 268266 item 2 (already correct).
    let document = manifest(
        MODE,
        r#"<server name="Good" group="g" ips_entry="192.0.2.10"/>"#,
    )
    .replace(
        &bootstrap_attributes(),
        r#"auth_rsa_exponent="" auth_rsa_modulus="AA==""#,
    )
    .replace("http://192.0.2.1", "ftp://user:pw@example");
    let m = parse_manifest(&document).expect("servers survive an unusable bootstrap");
    assert!(m.bootstrap.is_none());
    assert_eq!(m.servers.len(), 1);
}

#[test]
fn forum_d06_missing_pkexec_is_explained() {
    let missing = std::path::Path::new("/nonexistent/wave-test/pkexec");
    let error = wave_protocol::service::authorization_available(missing)
        .unwrap_err()
        .to_string();
    assert!(
        error.starts_with(wave_protocol::service::PKEXEC_MISSING),
        "{error}"
    );
    assert!(error.contains("sudo wave-vpn service-install"), "{error}");
    let present = tempfile::NamedTempFile::new().unwrap();
    assert!(wave_protocol::service::authorization_available(present.path()).is_ok());
}

/// Every POST fails; counts attempts and sets `flag` after the first.
struct Down {
    calls: std::sync::Mutex<usize>,
    flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl wave_protocol::Transport for Down {
    fn post(
        &self,
        _: &reqwest::Url,
        _: &std::collections::BTreeMap<String, String>,
    ) -> anyhow::Result<Vec<u8>> {
        *self.calls.lock().unwrap() += 1;
        self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
        anyhow::bail!("blackholed")
    }
}

#[test]
fn forum_d09_sign_in_cancel_stops_failover_alongside_the_session_flag() {
    use std::sync::{atomic::AtomicBool, Arc};
    // A sign-in carries the session's sign-out flag and its own Cancel /
    // deadline flag; either one stops failover, so a cancelled or timed-out
    // sign-in (or a sign-out) launches no further endpoint attempt.
    for which in ["session", "own"] {
        let session = Arc::new(AtomicBool::new(false));
        let own = Arc::new(AtomicBool::new(false));
        let down = Arc::new(Down {
            calls: Default::default(),
            flag: if which == "session" {
                session.clone()
            } else {
                own.clone()
            },
        });
        let Err(error) = wave_protocol::Client::with_transport(down.clone())
            .cancellable(session)
            .cancellable(own)
            .pausing(|_| ())
            .login("synthetic", "synthetic")
        else {
            panic!("a blackholed sign-in cannot succeed");
        };
        assert!(error.is::<wave_protocol::Cancelled>(), "{which}: {error}");
        assert_eq!(*down.calls.lock().unwrap(), 1, "{which}");
    }
}

#[test]
fn forum_d09_sign_in_deadline_is_45_seconds_with_shared_copy() {
    assert_eq!(
        wave_protocol::SIGN_IN_DEADLINE,
        std::time::Duration::from_secs(45)
    );
    // Identical on Android (AppViewModel.SIGN_IN_TIMED_OUT).
    assert_eq!(
        wave_protocol::SIGN_IN_TIMED_OUT,
        "Sign-in timed out after 45 seconds. Check your network and retry shortly."
    );
}
