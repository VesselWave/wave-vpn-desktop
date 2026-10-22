// SPDX-License-Identifier: GPL-3.0-or-later
use serde_json::json;
use std::time::Duration;
use wave_protocol::{checks, diagnostics, lifecycle, protection};
#[test]
fn provider_challenges_reject_wrong_identity_and_stale_dns_receipts() {
    let expected = "10.1.2.3".parse().unwrap();
    assert!(checks::verify_ip(&json!({"ip":"10.1.2.3"}), expected).is_ok());
    for reply in [
        json!({"ip":"203.0.113.8"}),
        json!({"ip":"garbage"}),
        json!({}),
    ] {
        assert!(checks::verify_ip(&reply, expected).is_err());
    }
    assert!(checks::verify_dns(&json!({"dns":"fresh-token"}), "fresh-token").is_ok());
    assert!(checks::verify_dns(&json!({"dns":"old-token"}), "fresh-token").is_err());
    assert!(checks::verify_dns(&json!({}), "fresh-token").is_err());
    let mut check = checks::ProviderCheck {
        server: "Comae".into(),
        exits: vec!["192.0.2.1".parse().unwrap()],
        domain: "airservers.org:89".into(),
        dns_query: "{hash}.checkdns.airvpn.org".into(),
    };
    assert!(check.validate().is_ok());
    check.domain = "localhost:1234".into();
    assert!(check.validate().is_err());
}
#[test]
fn firewall_query_distinguishes_real_policy_foreign_tables_and_missing_chains() {
    let mut table = json!({"nftables":[{"table":{"family":"inet","name":"wave_vpn","comment":"Managed by Wave VPN"}}, {"chain":{"table":"wave_vpn","name":"output","hook":"output","type":"filter","prio":-10,"policy":"drop"}}]});
    assert!(protection::table_enabled(&table).unwrap());
    table["nftables"][1]["chain"]["policy"] = json!("accept");
    assert!(!protection::table_enabled(&table).unwrap());
    table["nftables"][0]["table"]["comment"] = json!("foreign owner");
    assert!(protection::table_enabled(&table).is_err());
    assert!(protection::table_enabled(&json!({"nftables":[]})).is_err());
}
#[test]
fn diagnostics_exports_preview_exactly_without_overwrite_or_private_input() {
    let context = diagnostics::Context {
        connected: true,
        checks: checks::Results {
            dns: checks::Outcome::Failed,
            ..Default::default()
        },
        last_failure: diagnostics::Failure::SignInNetwork,
        ..Default::default()
    };
    let report = diagnostics::report(&context);
    assert_eq!(
        report["session"]["protection"]["enabled"],
        serde_json::Value::Null
    );
    assert_eq!(report["session"]["checks"]["dns"], "failed");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.json");
    diagnostics::export_report(&path, &report).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        serde_json::to_vec_pretty(&report).unwrap()
    );
    assert!(diagnostics::export_report(&path, &json!({"replaced":true})).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
#[test]
fn retries_are_rate_bounded_and_suspend_is_detected_with_either_clock() {
    assert_eq!(lifecycle::retry_delay(0), Duration::from_secs(3));
    assert_eq!(lifecycle::retry_delay(3), Duration::from_secs(24));
    assert_eq!(lifecycle::retry_delay(u32::MAX), Duration::from_secs(120));
    assert!(lifecycle::resumed(
        Duration::from_secs(60),
        Duration::from_secs(1)
    ));
    assert!(lifecycle::resumed(
        Duration::from_secs(1),
        Duration::from_secs(60)
    ));
    assert!(!lifecycle::resumed(
        Duration::from_secs(1),
        Duration::from_secs(1)
    ));
    assert!(
        wave_protocol::sign_in_error(&wave_protocol::AccountRejected.into()).contains("rejected")
    );
    assert!(
        wave_protocol::sign_in_error(&anyhow::anyhow!("private URL/account material"))
            .contains("Recover")
    );
    assert!(
        !wave_protocol::sign_in_error(&anyhow::anyhow!("private URL/account material"))
            .contains("private URL")
    );
}

#[test]
fn provider_failures_remain_actionable_after_the_worker_stops() {
    let mut results = checks::Results::default();
    assert!(results.issue().is_none());
    results.ipv4 = checks::Outcome::Verified;
    results.ipv6 = checks::Outcome::Unavailable;
    assert!(results.issue().unwrap().contains("IPv6"));
    results.dns = checks::Outcome::Failed;
    assert!(results.issue().unwrap().contains("Recover"));
}

#[test]
fn account_without_devices_requests_account_action_instead_of_network_recovery() {
    let error = wave_protocol::parse_account("<user login=\"tester\"><keys/></user>", "tester")
        .err()
        .unwrap();
    assert!(error.is::<wave_protocol::AccountDevicesMissing>());
    let message = wave_protocol::sign_in_error(&error);
    assert!(message.contains("device") && !message.contains("Recover"));
}
