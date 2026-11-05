// SPDX-License-Identifier: GPL-3.0-or-later
use wave_protocol::{firewall, options::Options, transport::parse_stats};
#[test]
fn privileged_inputs_reject_injection_and_invalid_ranges() {
    for (key, value) in [
        ("entry_interface", "eth0\"; accept"),
        ("entry_interface", "eth0,eth1"),
        ("forward_interfaces", "wavevpn0"),
        ("allowlist", "1.2.3.4/99"),
        ("routes", "0.0.0.0/0,maybe"),
        ("dns", "1.1.1.1\nPostUp=evil"),
        ("mtu", "0"),
        ("port", "65536"),
        ("incoming", "permit"),
        ("dns_services", "--help"),
        ("unknown", "true"),
    ] {
        let mut o = Options::default();
        o.set(key, value.into());
        assert!(o.validate().is_err(), "accepted {key}={value}");
        assert!(firewall::policy(&o, None, true).is_err());
    }
}
#[test]
fn explicit_bypass_routes_and_forwarding_are_validated() {
    let mut o = Options::default();
    o.set("routes", "192.0.2.0/24,out;2001:db8::/32,in".into());
    o.set("forward_interfaces", "eth1,br0".into());
    o.validate().unwrap();
    let policy = firewall::policy(&o, None, true).unwrap();
    assert!(policy.contains("ip daddr 192.0.2.0/24 accept"));
    assert!(policy.contains("iifname \"br0\" oifname \"wavevpn0\" masquerade"));
}
#[test]
fn wireguard_peer_statistics_are_not_confused_with_interface_secrets() {
    let stats = parse_stats(
        "private public 12345 51820\npeer psk 192.0.2.1:123 0.0.0.0/0 1700000000 123456 789 15\n",
    )
    .unwrap();
    assert_eq!(
        (stats.handshake, stats.rx, stats.tx),
        (1700000000, 123456, 789)
    );
    assert!(parse_stats("private public 0 off\n").is_err());
    assert!(parse_stats("interface\ninvalid peer\n").is_err());
}

#[test]
fn desktop_migration_removes_hidden_bypasses_and_keeps_supported_preferences() {
    let mut legacy = Options::default();
    for (key, value) in [
        ("netlock", "true"),
        ("allow_ping", "true"),
        ("allowlist", "0.0.0.0/0"),
        ("routes", "0.0.0.0/0,out"),
        ("forward_interfaces", "eth0"),
        ("theme", "dark"),
    ] {
        legacy.set(key, value.into());
    }
    let desktop = legacy.desktop();
    desktop.validate().unwrap();
    // A saved Network Lock choice carries over; new installs start with it off.
    assert!(desktop.on("netlock"));
    assert!(!Options::default().on("netlock"));
    assert!(!desktop.on("allow_ping"));
    assert!(!Options::default().on("allow_lan"));
    for key in ["allowlist", "routes", "forward_interfaces"] {
        assert_eq!(desktop.get(key), "");
    }
    assert_eq!(desktop.get("theme"), "dark");
    assert_eq!(
        wave_protocol::options::FIELDS
            .iter()
            .filter(|f| wave_protocol::options::desktop_field(f.key))
            .count(),
        5
    );
}
