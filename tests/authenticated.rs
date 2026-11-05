// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit live test. Credential JSON arrives only on stdin; private worker
//! requests go to a new mode-0600 file, never test output or process arguments.
use anyhow::{ensure, Context, Result};
use std::io::{Read, Write};
use wave_protocol::{credentials::Credentials, options::Options, transport::Request, Client};
use zeroize::Zeroizing;

#[test]
#[ignore = "Requires credential JSON on stdin and WAVE_AUTH_OUTPUT for private requests"]
fn authenticated_profiles_for_isolated_linux_verification() -> Result<()> {
    let destination = std::env::var_os("WAVE_AUTH_OUTPUT").context("Set private output path")?;
    let mut bytes = Zeroizing::new(Vec::new());
    std::io::stdin().take(16385).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 16384, "Credential input too large");
    let credentials: Credentials =
        serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("Invalid credential JSON"))?;
    let (account, manifest) = Client::new()?.login(&credentials.login, &credentials.password)?;
    let mode = manifest.modes.first().context("No WireGuard mode")?;
    let mut servers: Vec<_> = manifest.servers.iter().filter(|s| s.available()).collect();
    servers.sort_by(|a, b| manifest.score(a).total_cmp(&manifest.score(b)));
    let mut requests = Vec::new();
    // The live suite checks that nothing escapes the tunnel, so it needs the lock.
    let mut locked = Options::default();
    locked.set("netlock", "true".into());
    for server in servers {
        if let Ok(profile) = account.profile(server, "", mode, false) {
            requests.push(Request {
                profile,
                options: locked.clone(),
                checks: manifest.provider_check(server),
            });
            println!(
                "Selected public server {} for live verification",
                server.name
            );
            if requests.len() == 2 {
                break;
            }
        }
    }
    ensure!(requests.len() == 2, "Need two available WireGuard servers");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(destination)?;
    let encoded = Zeroizing::new(serde_json::to_vec(&requests)?);
    file.write_all(&encoded)?;
    file.sync_all()?;
    println!("PASS authenticated account, live manifest and two validated WireGuard requests; no secrets logged");
    Ok(())
}
