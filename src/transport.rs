// SPDX-License-Identifier: GPL-3.0-or-later
//! Platform-neutral transport boundary. Protocol and GUI do not depend on Linux tools.
use crate::{options::Options, TunnelProfile};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub profile: TunnelProfile,
    pub options: Options,
    #[serde(default)]
    pub checks: Option<crate::checks::ProviderCheck>,
}
#[derive(Default)]
pub struct Stats {
    pub handshake: u64,
    pub rx: u64,
    pub tx: u64,
}
pub fn parse_stats(text: &str) -> Result<Stats> {
    let line = text.lines().nth(1).context("WireGuard peer missing")?;
    let c = line.split_whitespace().collect::<Vec<_>>();
    ensure!(c.len() >= 8, "Incomplete peer statistics");
    Ok(Stats {
        handshake: c[4].parse()?,
        rx: c[5].parse()?,
        tx: c[6].parse()?,
    })
}

#[cfg(target_os = "linux")]
#[path = "platform/linux.rs"]
mod backend;
#[cfg(target_os = "linux")]
pub use backend::{lock_action, network_status, recover, worker};
#[cfg(not(target_os = "linux"))]
pub fn worker() -> Result<()> {
    anyhow::bail!("VPN connections require a platform backend; currently Linux is supported")
}
#[cfg(not(target_os = "linux"))]
pub fn recover() -> Result<()> {
    anyhow::bail!("Network recovery is currently supported on Linux")
}
#[cfg(not(target_os = "linux"))]
pub fn lock_action(_action: &str) -> Result<()> {
    anyhow::bail!("Persistent network lock is currently supported on Linux")
}

#[cfg(not(target_os = "linux"))]
pub fn network_status() -> Result<()> {
    anyhow::bail!("Network status requires Linux")
}
