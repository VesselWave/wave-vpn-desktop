// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit demo mode for visual comparisons. No network requests or privileged operations.
use super::*;
pub fn setup(app: &mut Wave) {
    app.protection_known = true;
    app.prefs = Preferences::default();
    app.draft = app.prefs.options.clone();
    app.login = "Preview account".into();
    app.prefs.device = "Desktop".into();
    let key = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";
    let xml = format!(
        r#"<user login="Preview account" wg_public_key="{key}"><keys><key name="Desktop" wg_private_key="{key}" wg_preshared="{key}" wg_ipv4="10.1.2.3/32" wg_ipv6="fd00::1/128" wg_dns_ipv4="10.1.0.1" wg_dns_ipv6="fd00::2"/><key name="Phone" wg_private_key="{key}" wg_preshared="{key}" wg_ipv4="10.1.2.4/32" wg_dns_ipv4="10.1.0.1"/></keys></user>"#
    );
    app.account = Some(wave_protocol::parse_account(&xml, "Preview account").unwrap());
    let mut m =
        wave_protocol::parse_manifest(include_str!("../protocol-data/fixtures/eddie-manifest.xml"))
            .unwrap();
    let sample = [
        ("Alhena", "nl", "Amsterdam", 24, 18),
        ("Vega", "de", "Frankfurt", 42, 30),
        ("Castor", "ch", "Zurich", 31, 26),
        ("Rigel", "se", "Stockholm", 18, 41),
        ("Deneb", "gb", "London", 55, 22),
        ("Mirfak", "ca", "Toronto", 37, 88),
        ("Sirius", "us", "New York", 68, 95),
        ("Antares", "sg", "Singapore", 46, 170),
    ];
    m.servers.truncate(sample.len());
    for (s, (name, code, city, load, ping)) in m.servers.iter_mut().zip(sample) {
        s.name = name.into();
        s.country = code.into();
        s.location = city.into();
        s.bandwidth = load * 625_000;
        s.capacity = 1000;
        s.users = 100;
        s.users_max = 1000;
        s.score_base = 0.;
        s.warning_closed.clear();
        s.warning_open.clear();
        s.entries = vec![
            "192.0.2.1".parse().unwrap(),
            "192.0.2.2".parse().unwrap(),
            "192.0.2.3".parse().unwrap(),
        ];
        app.pings.insert(s.name.clone(), ping);
    }
    app.manifest = Some(m);
    app.log(Level::Info, "Preview data — no real VPN connection".into());
    match std::env::var("WAVE_PREVIEW_SCENE").as_deref() {
        Ok("connected") => {
            app.active_server = app
                .manifest
                .as_ref()
                .and_then(|m| m.servers.first())
                .cloned();
            app.status = "Connected".into();
            app.since = Some(Instant::now() - Duration::from_secs(3661));
            app.rx = 64_000_000;
            app.tx = 6_000_000;
            app.rx_rate = 2_400_000.;
            app.tx_rate = 320_000.;
            app.connections = 1;
            app.guarded = true;
            let verified = wave_protocol::checks::Outcome::Verified;
            app.checks = wave_protocol::checks::Results { ipv4: verified, ipv6: verified, dns: verified };
        }
        Ok("diagnostics") => {
            app.tab = Tab::Logs;
            let _ = app.update(Message::Diagnostics);
        }
        Ok("unknown") => {
            app.protection_known = false;
            app.tab = Tab::Settings;
            app.section = Section::Lock;
        }
        Ok("logged-out") => {
            app.account = None;
            app.manifest = None;
            app.tab = Tab::Settings;
        }
        Ok("error") => {
            app.error = Some("Connection interrupted. Network lock remains active.".into());
            app.guarded = true;
        }
        _ => (),
    }
}
pub fn tick(app: &mut Wave) -> Task<Message> {
    let tick = app.capture_tick;
    app.capture_tick += 1;
    if std::env::var_os("WAVE_CAPTURE_DIR").is_none() {
        return Task::none();
    }
    if std::env::var_os("WAVE_PREVIEW_SCENE").is_some() {
        if tick == 20 {
            return iced::window::latest()
                .and_then(iced::window::screenshot)
                .map(Message::Capture);
        }
        return Task::none();
    }
    let step = tick / 40;
    if step >= 4 + Section::ALL.len() {
        return Task::none();
    }
    match step {
        0 => app.tab = Tab::Overview,
        1 => {
            app.tab = Tab::Servers;
            app.server_view = ServerView::All;
        }
        2 => {
            app.tab = Tab::Servers;
            app.server_view = ServerView::Countries;
        }
        3 => app.tab = Tab::Logs,
        n => {
            app.tab = Tab::Settings;
            app.section = Section::ALL[n - 4];
        }
    }
    if tick % 40 == 20 {
        return iced::window::latest()
            .and_then(iced::window::screenshot)
            .map(Message::Capture);
    }
    Task::none()
}
pub fn save(app: &Wave, s: iced::window::Screenshot) {
    if let Some(dir) = std::env::var_os("WAVE_CAPTURE_DIR") {
        let dir = std::path::PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&dir);
        let name = std::env::var("WAVE_PREVIEW_SCENE")
            .unwrap_or_else(|_| (app.capture_tick / 40).to_string());
        let path = dir.join(format!("new-{name}.png"));
        if let Ok(file) = std::fs::File::create(path) {
            let mut enc = png::Encoder::new(file, s.size.width, s.size.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            if let Ok(mut writer) = enc.write_header() {
                let _ = writer.write_image_data(&s.rgba);
            }
        }
    }
}
