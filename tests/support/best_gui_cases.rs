// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
#[test]
fn best_country_explicit_reconnect_and_recent_failure_choices() {
    let mut app = Wave::new();
    app.preview = true;
    app.prefs.options = Options::default();
    let mut m = wave_protocol::parse_manifest(include_str!(
        "../../protocol-data/fixtures/eddie-manifest.xml"
    ))
    .unwrap();
    let base = m.servers[0].clone();
    m.servers = [("US", "us", 24), ("NL", "nl", 110), ("SG", "sg", 230)]
        .into_iter()
        .map(|(name, country, ms)| {
            let mut s = base.clone();
            s.name = name.into();
            s.country = country.into();
            s.location = name.into();
            s.capacity = 1000;
            s.bandwidth = 0;
            s.users = 0;
            s.users_max = 100;
            s.score_base = 0.;
            s.warning_open.clear();
            s.warning_closed.clear();
            app.latency.insert(name.into(), latency::Ping::Measured(ms));
            s
        })
        .collect();
    app.latency.set_locations(&m.servers);
    app.manifest = Some(m);
    app.prefs.target = Target::Best;
    assert_eq!(app.selected().unwrap().name, "US");
    app.prefs.target = Target::Country("nl".into());
    assert_eq!(app.selected().unwrap().name, "NL");
    app.prefs.target = Target::Server("SG".into());
    assert_eq!(app.selected().unwrap().name, "SG");
    app.prefs.target = Target::Best;
    app.penalties
        .insert("US".into(), Instant::now() + Duration::from_secs(60));
    assert_eq!(app.selected().unwrap().name, "NL");
    app.session_server = Some("SG".into());
    app.prefs.options.set("locklast", "true".into());
    assert_eq!(app.selected().unwrap().name, "SG");
}
