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
    /// Preview or export allowlisted diagnostics; never includes raw logs or secrets.
    Diagnostics {
        #[arg(long)]
        output: Option<std::path::PathBuf>,
    },
    #[command(hide = true)]
    TransportWorker,
    /// Install/upgrade the networking service for the invoking desktop user.
    ServiceInstall {
        #[arg(long)]
        uid: Option<u32>,
    },
    ServiceUninstall,
    #[command(hide = true)]
    ServiceServe,
    #[command(hide = true)]
    ServiceClient {
        action: String,
    },
    /// Restore only networking resources owned by Wave.
    Recover,
    LockInstall,
    LockUninstall,
    LockEnable,
    LockDisable,
    #[command(hide = true)]
    LockRestore,
}
fn main() -> Result<()> {
    match Args::parse().command {
        Action::Diagnostics { output } => {
            if let Some(path) = output {
                diagnostics::export(&path)?;
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&diagnostics::snapshot())?
                );
            }
            Ok(())
        }
        Action::TransportWorker => transport::worker(),
        Action::ServiceInstall { uid } => service::install(uid),
        Action::ServiceUninstall => service::uninstall(),
        Action::ServiceServe => service::serve(),
        Action::ServiceClient { action } => service::client(&action),
        Action::Recover => transport::recover(),
        Action::LockInstall => transport::lock_action("lock-install"),
        Action::LockUninstall => transport::lock_action("lock-uninstall"),
        Action::LockEnable => transport::lock_action("lock-enable"),
        Action::LockDisable => transport::lock_action("lock-disable"),
        Action::LockRestore => transport::lock_action("lock-restore"),
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
