// SPDX-License-Identifier: GPL-3.0-or-later
//! Independent synthetic recheck of findings F1-F5, now asserting the intended
//! invariants. See docs/STAFF-REGRESSION-RECHECK.md.
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

/// F1: supplied pairs are scoped to the authorized client UID; with no
/// known UID they are not installed at all. The helper still cannot prove
/// provider provenance (documented boundary), so only scope is asserted.
#[test]
fn recheck_supplied_exemptions_are_limited_to_the_client_uid() {
    let input =
        LockInput::parse(br#"{"options":{},"bootstrap":[{"ip":"1.1.1.1","port":443}]}"#).unwrap();
    let mut options = input.options;
    options.set("allow_lan", "false".into());
    let rules = firewall::policy(&options, None, true, &input.bootstrap, Some(1000)).unwrap();
    assert!(rules.contains("policy drop"));
    assert!(rules.contains("meta skuid 1000 ip daddr 1.1.1.1 tcp dport 443 accept"));
    // Every bootstrap accept rule carries the owner match.
    for line in rules
        .lines()
        .filter(|l| l.contains("tcp dport") && l.ends_with("accept"))
    {
        assert!(line.starts_with("meta skuid 1000 "), "unscoped: {line}");
    }
    let unknown = firewall::policy(&options, None, true, &input.bootstrap, None).unwrap();
    assert!(
        !unknown.contains("1.1.1.1"),
        "no UID: supplied pairs omitted"
    );
    assert!(
        unknown.contains("63.33.78.166"),
        "compiled-in fallback kept"
    );
}

/// F5: non-public IPv6 (and IPv4) categories, including transition and
/// translation forms, never validate or reach the policy.
#[test]
fn recheck_non_public_addresses_are_refused() {
    for ip in [
        "fec0::1",
        "100::1",
        "64:ff9b::808:808",
        "64:ff9b:1::1",
        "2002:808:808::1",
        "2001::1",
        "2001:2::1",
        "2001:20::1",
        "2001:db8::1",
        "3fff::1",
        "::ffff:8.8.8.8",
        "::8.8.8.8",
        "fc00::1",
        "fe80::1",
        "ff02::1",
        "::1",
        "::",
        "4000::1",
        "192.0.0.9",
        "192.0.2.1",
        "192.88.99.1",
        "198.18.0.1",
        "198.51.100.1",
        "203.0.113.1",
        "100.64.0.1",
        "240.0.0.1",
        "255.255.255.255",
        "0.1.2.3",
    ] {
        let exemption = Exemption {
            ip: ip.parse().unwrap(),
            port: 443,
        };
        assert!(exemption.validate().is_err(), "{ip} accepted");
        assert!(
            firewall::policy(&Options::default(), None, true, &[exemption], Some(1000)).is_err()
        );
    }
    for ip in [
        "8.8.8.8",
        "1.1.1.1",
        "2a03:b0c0:0:1010::9b:c001",
        "2606:4700::1111",
        "2001:200::1",
    ] {
        assert!(
            bootstrap::public_unicast(ip.parse().unwrap()),
            "{ip} refused"
        );
    }
}

/// F2: one global selection. Every pinned client address is in the helper's
/// exemption set, and every hostname keeps at least one address in budget.
#[test]
fn recheck_hostname_pins_are_a_subset_of_the_exemption_set() {
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
    let stored: usize = resolved.values().map(Vec::len).sum();
    assert_eq!(
        stored,
        bootstrap::MAX_EXEMPTIONS,
        "only budgeted addresses are kept"
    );
    for url in &b.urls {
        let url = bootstrap::endpoint(url).unwrap();
        let pins = b.pinned(&url, &resolved).expect("every host keeps a pin");
        assert!(pins.len() >= 3);
        assert!(pins.iter().all(|s| exemptions.contains(&Exemption {
            ip: s.ip(),
            port: s.port()
        })));
    }
    // An oversized previously stored map is still filtered to the selection.
    let mut oversized = resolved.clone();
    oversized.insert(
        "bootstrap4.example".into(),
        (1..=4)
            .map(|i| format!("9.9.9.{i}").parse().unwrap())
            .collect(),
    );
    let allowed = b.exemptions(&oversized);
    let url = bootstrap::endpoint(&b.urls[4]).unwrap();
    for pin in b.pinned(&url, &oversized).unwrap() {
        assert!(allowed.contains(&Exemption {
            ip: pin.ip(),
            port: pin.port()
        }));
    }
}

/// F4: a late equal/unknown-time write from a session that started before a
/// later write is refused; the session that saw the current revision may
/// still re-resolve hostnames; a strictly newer time always wins.
#[test]
fn recheck_equal_or_unknown_manifest_time_is_fenced_by_revision() {
    for issued in [0, 1_000] {
        let dir = tempfile::tempdir().unwrap();
        let store = BootstrapStore::in_directory(dir.path());
        let old_base = store.revision();
        let new_base = store.revision();
        let newer_session = configuration("https://8.8.8.8");
        let late_old_session = configuration("https://1.1.1.1");
        assert_eq!(
            store
                .save_fenced(&newer_session, issued, &Resolved::new(), new_base, &|| true)
                .unwrap(),
            Saved::Written
        );
        assert_eq!(
            store
                .save_fenced(
                    &late_old_session,
                    issued,
                    &Resolved::new(),
                    old_base,
                    &|| true
                )
                .unwrap(),
            Saved::Stale
        );
        assert_eq!(store.load(), Some(newer_session.clone()));
        // Re-resolution of the same manifest by a current session is allowed.
        let mut host = configuration("https://boot.example");
        host.urls.push("https://8.8.8.8".into());
        let base = store.revision();
        let first: Resolved =
            [("boot.example".to_owned(), vec!["8.8.4.4".parse().unwrap()])].into();
        store
            .save_fenced(&host, issued, &first, base, &|| true)
            .unwrap();
        let base = store.revision();
        let second: Resolved =
            [("boot.example".to_owned(), vec!["8.8.4.5".parse().unwrap()])].into();
        assert_eq!(
            store
                .save_fenced(&host, issued, &second, base, &|| true)
                .unwrap(),
            Saved::Written
        );
        assert_eq!(store.latest().resolved, second);
        // A strictly newer manifest time wins even from an old base.
        assert_eq!(
            store
                .save_fenced(
                    &late_old_session,
                    issued + 1,
                    &Resolved::new(),
                    old_base,
                    &|| true
                )
                .unwrap(),
            Saved::Written
        );
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
            0,
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
