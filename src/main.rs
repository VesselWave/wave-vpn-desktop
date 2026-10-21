// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::Write;
use wave_protocol::*;
use zeroize::Zeroize;

#[derive(Parser)]
#[command(about = "Wave VPN — unofficial GPLv3 AirVPN client")]
struct Args {
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Fetch the server list without saving account credentials.
    Servers {
        #[arg(long)]
        login: String,
    },
    /// Generate a private WireGuard configuration for an existing VPN manager.
    Profile {
        #[arg(long)]
        login: String,
        #[arg(long)]
        server: String,
        #[arg(long, default_value = "")]
        device: String,
        #[arg(long, default_value_t = 1637)]
        port: u16,
        #[arg(long)]
        ipv6: bool,
        #[arg(long)]
        output: std::path::PathBuf,
    },
    #[command(hide = true)]
    TransportWorker,
}
fn main() -> Result<()> {
    match Args::parse().command {
        Action::TransportWorker => transport::worker(),
        action => {
            let login = match &action {
                Action::Servers { login } | Action::Profile { login, .. } => login,
                _ => unreachable!(),
            };
            let mut password = rpassword::prompt_password("AirVPN password: ")?;
            let response = Client::new()?.login(login, &password);
            password.zeroize();
            let (account, manifest) = response?;
            match action {
                Action::Servers { .. } => {
                    for s in &manifest.servers {
                        println!(
                            "{}\t{}\t{}\t{:.0}%\t{}",
                            s.name,
                            s.country,
                            s.location,
                            s.load(),
                            if s.available() { "available" } else { "closed" }
                        );
                    }
                }
                Action::Profile {
                    server,
                    device,
                    port,
                    ipv6,
                    output,
                    ..
                } => {
                    let s = manifest
                        .servers
                        .iter()
                        .find(|s| s.name == server)
                        .context("Unknown server")?;
                    let m = manifest
                        .modes
                        .iter()
                        .find(|m| m.port == port)
                        .context("Unsupported WireGuard port")?;
                    let mut text = account.profile(s, &device, m, ipv6)?.config()?;
                    let mut options = std::fs::OpenOptions::new();
                    options.write(true).create_new(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::OpenOptionsExt;
                        options.mode(0o600);
                    }
                    let mut file = options.open(output)?;
                    let result = file.write_all(text.as_bytes());
                    text.zeroize();
                    result?;
                }
                _ => unreachable!(),
            }
            Ok(())
        }
    }
}
