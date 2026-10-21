// SPDX-License-Identifier: GPL-3.0-or-later
//! Allowlisted diagnostics. Never reads profiles, credentials, preferences or raw logs.
use anyhow::Result;
use serde_json::{json, Value};
use std::{io::Write, path::Path};
pub fn snapshot() -> Value {
    json!({
        "schema": 1,
        "application": "Wave VPN",
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "linux_backend": cfg!(target_os="linux"),
        "redaction": "Account names, credentials, keys, addresses, preferences, environment, filesystem paths and raw logs are omitted.",
        "support_status": "development; see docs/PARITY.md for release gates"
    })
}
pub fn export(path: &Path) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(&serde_json::to_vec_pretty(&snapshot())?)?;
    file.sync_all()?;
    Ok(())
}
