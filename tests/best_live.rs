// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in five-location quality sample. Credentials stdin; private requests mode 0600.
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
};
use wave_protocol::{
    credentials::Credentials, latency::Ping, options::Options, transport::Request, Client,
};
#[test]
#[ignore = "Explicit five-server live sampling only; stdin credentials and WAVE_BEST_OUTPUT required"]
fn five_location_profiles() -> anyhow::Result<()> {
    let mut input = zeroize::Zeroizing::new(String::new());
    std::io::stdin().take(16385).read_to_string(&mut input)?;
    let creds: Credentials = serde_json::from_str(&input)?;
    let (a, m) = Client::new()?.login(&creds.login, &creds.password)?;
    let mode = &m.modes[0];
    let mut rows = Vec::new();
    for country in ["us", "ca", "nl", "sg", "jp"] {
        let Some(s) = m
            .servers
            .iter()
            .filter(|s| s.country == country && s.available() && s.warning_open.is_empty())
            .min_by_key(|s| m.desktop_score(s, Ping::Unknown, 0))
        else {
            continue;
        };
        let Ok(profile) = a.profile(s, "", mode, false) else {
            continue;
        };
        let request = Request {
            profile,
            options: Options::default(),
            checks: None,
            bootstrap: vec![],
        };
        rows.push(serde_json::json!({"name":s.name,"country":s.country,"load":s.load(),"capacity":s.capacity,"users":s.users,"users_max":s.users_max,"base":s.score_base,"factors":m.factors,"entry":s.entries.iter().find(|ip|ip.is_ipv4()),"request":request}));
    }
    anyhow::ensure!(!rows.is_empty(), "No sample candidates");
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(std::env::var("WAVE_BEST_OUTPUT")?)?;
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&rows)?);
    f.write_all(&bytes)?;
    println!(
        "Prepared {} bounded regional samples; no private data logged",
        rows.len()
    );
    Ok(())
}
