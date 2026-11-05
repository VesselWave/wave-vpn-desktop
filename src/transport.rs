// SPDX-License-Identifier: GPL-3.0-or-later
//! Platform-neutral transport boundary. Protocol and GUI do not depend on Linux tools.
use crate::{
    bootstrap::{validate_exemptions, Exemption, MAX_EXEMPTIONS},
    options::Options,
    TunnelProfile,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub profile: TunnelProfile,
    pub options: Options,
    #[serde(default)]
    pub checks: Option<crate::checks::ProviderCheck>,
    /// Latest-manifest bootstrap IP/port pairs Network Lock must allow.
    #[serde(default)]
    pub bootstrap: Vec<Exemption>,
}

/// Input of the privileged lock actions: the desktop's options and the
/// latest-manifest bootstrap exemptions. The CLI may still pass bare options.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockInput {
    pub options: Options,
    #[serde(default)]
    pub bootstrap: Vec<Exemption>,
}
impl LockInput {
    /// Parse and validate untrusted helper input. Unsafe or too many
    /// exemptions are refused outright, never silently trimmed.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= 65536, "Options too large");
        let input = if bytes.iter().all(u8::is_ascii_whitespace) {
            Self::default()
        } else if let Ok(input) = serde_json::from_slice::<Self>(bytes) {
            input
        } else {
            Self {
                options: serde_json::from_slice(bytes).context("Invalid lock input")?,
                bootstrap: vec![],
            }
        };
        input.options.validate()?;
        validate_exemptions(&input.bootstrap)?;
        Ok(input)
    }
}

/// The boot-persistent Network Lock configuration (root-only on disk).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersistentLock {
    pub options: Options,
    pub enabled: bool,
    /// Latest-manifest exemptions, restored at boot with the lock.
    #[serde(default)]
    pub bootstrap: Vec<Exemption>,
}
impl PersistentLock {
    /// Options must validate. Exemptions that do not are dropped (fewer
    /// exemptions only block more), so a damaged list cannot disable the lock.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut lock: Self =
            serde_json::from_slice(bytes).context("Invalid persistent lock configuration")?;
        lock.options.validate()?;
        lock.bootstrap.retain(|e| e.validate().is_ok());
        lock.bootstrap.truncate(MAX_EXEMPTIONS);
        Ok(lock)
    }
}
/// The worker's final `failure` event. Setup errors and observed verification
/// mismatches are not retryable: reconnecting cannot fix them and would only
/// add load. Other connection errors may be retried with backoff.
pub fn failure_event(configured: bool, error: &anyhow::Error) -> serde_json::Value {
    let (retryable, category) = if error.is::<crate::checks::VerificationFailed>() {
        (false, "verification")
    } else if configured {
        (true, "connection")
    } else {
        (false, "configuration")
    };
    serde_json::json!({"event":"failure", "retryable":retryable, "category":category})
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
