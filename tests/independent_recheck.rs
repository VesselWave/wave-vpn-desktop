// SPDX-License-Identifier: GPL-3.0-or-later
//! Independent synthetic recheck. `observed_*` cases characterize review
//! findings, not desired security contracts. See docs/STAFF-REGRESSION-RECHECK.md.
use wave_protocol::{
    bootstrap::{self, Exemption, Resolved, Saved},
    firewall,
    options::Options,
    transport::LockInput,
    Bootstrap, BootstrapStore,
};

fn configuration(url: &str) -> Bootstrap {
    let mut b = Bootstrap::eddie();
    b.urls = vec![url.into()];
    b
}

#[test]
fn observed_helper_accepts_caller_chosen_public_egress_without_provider_proof() {
    let input =
        LockInput::parse(br#"{"options":{},"bootstrap":[{"ip":"1.1.1.1","port":443}]}"#).unwrap();
    let mut options = input.options;
    options.set("allow_lan", "false".into());
    let rules = firewall::policy(&options, None, true, &input.bootstrap).unwrap();
    assert!(rules.contains("policy drop"));
    assert!(rules.contains("ip daddr 1.1.1.1 tcp dport 443 accept"));
    // It applies to every process, protocol payload and HTTP path at this pair.
    assert!(!rules.contains("meta skuid"));
}

#[test]
fn observed_non_public_ipv6_categories_are_accepted() {
    for ip in ["fec0::1", "100::1"] {
        let exemption = Exemption {
            ip: ip.parse().unwrap(),
            port: 443,
        };
        exemption.validate().unwrap();
        assert!(
            firewall::policy(&Options::default(), None, true, &[exemption])
                .unwrap()
                .contains(&format!("ip6 daddr {ip} tcp dport 443 accept"))
        );
    }
}

#[test]
fn observed_hostname_pins_exceed_the_actual_firewall_exemption_set() {
    let mut b = configuration("https://bootstrap0.example:4443");
    b.urls = (0..5)
        .map(|n| format!("https://bootstrap{n}.example:4443"))
        .collect();
    b.validate().unwrap();
    let resolved = bootstrap::resolve_hosts(&b, &Resolved::new(), &|name| {
        let n: u8 = name
            .strip_prefix("bootstrap")
            .unwrap()
            .split('.')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        Some(
            (1..=4)
                .map(|i| format!("8.8.{n}.{i}").parse().unwrap())
                .collect(),
        )
    });
    let exemptions = b.exemptions(&resolved);
    assert_eq!(exemptions.len(), bootstrap::MAX_EXEMPTIONS);
    let url = bootstrap::endpoint(b.urls.last().unwrap()).unwrap();
    let pins = bootstrap::pinned(&url, &resolved).unwrap();
    assert_eq!(pins.len(), 4);
    assert!(pins.iter().all(|s| !exemptions.contains(&Exemption {
        ip: s.ip(),
        port: s.port()
    })));
}

#[test]
fn observed_equal_or_unknown_manifest_time_allows_late_replacement() {
    for issued in [0, 1_000] {
        let dir = tempfile::tempdir().unwrap();
        let store = BootstrapStore::in_directory(dir.path());
        let newer_session = configuration("https://8.8.8.8");
        let late_old_session = configuration("https://1.1.1.1");
        store
            .save_fenced(&newer_session, issued, &Resolved::new(), &|| true)
            .unwrap();
        assert_eq!(
            store
                .save_fenced(&late_old_session, issued, &Resolved::new(), &|| true)
                .unwrap(),
            Saved::Written
        );
        assert_eq!(store.load(), Some(late_old_session));
    }
}

#[cfg(unix)]
#[test]
fn recheck_store_permissions_and_symlink_boundaries() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let dir = tempfile::tempdir().unwrap();
    let private = dir.path().join("private");
    let store = BootstrapStore::in_directory(&private);
    let b = configuration("https://8.8.8.8");
    store.save(&b).unwrap();
    assert_eq!(
        std::fs::metadata(&private).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(store.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let victim = dir.path().join("victim");
    std::fs::write(&victim, b"unchanged").unwrap();
    std::fs::remove_file(store.path()).unwrap();
    symlink(&victim, store.path()).unwrap();
    assert!(store.load().is_none());
    store.save(&b).unwrap(); // atomic replacement must not follow the symlink
    assert_eq!(std::fs::read(&victim).unwrap(), b"unchanged");
    std::fs::remove_file(private.join("bootstrap.lock")).unwrap();
    symlink(&victim, private.join("bootstrap.lock")).unwrap();
    assert!(store.save(&b).is_err());
    assert_eq!(std::fs::read(&victim).unwrap(), b"unchanged");
    let alias = dir.path().join("alias");
    symlink(&private, &alias).unwrap();
    assert!(BootstrapStore::in_directory(&alias).save(&b).is_err());
}

// A separate test process invokes this entry, exercising flock rather than
// just the process-wide Rust mutex. No network or privileged paths.
#[test]
fn recheck_process_writer_entry() {
    let Some(dir) = std::env::var_os("WAVE_RECHECK_STORE") else {
        return;
    };
    let issued: u64 = std::env::var("WAVE_RECHECK_ISSUED")
        .unwrap()
        .parse()
        .unwrap();
    BootstrapStore::in_directory(std::path::Path::new(&dir))
        .save_fenced(
            &configuration(&format!("https://8.8.8.{}", issued - 99)),
            issued,
            &Resolved::new(),
            &|| true,
        )
        .unwrap();
}

#[test]
fn recheck_separate_processes_preserve_strictly_newest_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let exe = std::env::current_exe().unwrap();
    let mut children: Vec<_> = (100..108)
        .rev()
        .map(|issued| {
            std::process::Command::new(&exe)
                .args(["--exact", "recheck_process_writer_entry"])
                .env("WAVE_RECHECK_STORE", dir.path())
                .env("WAVE_RECHECK_ISSUED", issued.to_string())
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in &mut children {
        assert!(child.wait().unwrap().success());
    }
    let latest = BootstrapStore::in_directory(dir.path()).latest();
    assert_eq!(latest.issued, 107);
    assert_eq!(latest.bootstrap.unwrap().urls, ["https://8.8.8.8"]);
}
