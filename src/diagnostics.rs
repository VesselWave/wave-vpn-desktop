// SPDX-License-Identifier: GPL-3.0-or-later
//! Allowlisted diagnostics. Never reads credentials, preferences, profiles or raw logs.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{io::Write, path::Path};
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    #[default]
    None,
    SignInNetwork,
    AccountRejected,
    Connection,
    Recovery,
    Service,
    Export,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub connected: bool,
    pub reconnecting: bool,
    pub protection: crate::protection::Status,
    pub checks: crate::checks::Results,
    pub last_failure: Failure,
    /// Destinations (count only) Network Lock lets the authorized user's
    /// processes reach outside the tunnel over TCP for AirVPN sign-in.
    #[serde(default)]
    pub bootstrap_exceptions: Option<usize>,
    /// A newer bootstrap set is not yet confirmed by the helper.
    #[serde(default)]
    pub bootstrap_exceptions_pending: bool,
}
fn available(name: &str) -> bool {
    ["/usr/sbin", "/usr/bin", "/sbin", "/bin"]
        .iter()
        .any(|dir| Path::new(dir).join(name).is_file())
}
pub fn report(context: &Context) -> Value {
    let dependencies: serde_json::Map<_, _> = [
        "wg",
        "ip",
        "nft",
        "getent",
        "systemctl",
        "resolvectl",
        "resolvconf",
        "secret-tool",
        "pkexec",
    ]
    .into_iter()
    .map(|name| (name.into(), json!(available(name))))
    .collect();
    json!({
        "schema": 2, "application": "Wave VPN", "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS, "architecture": std::env::consts::ARCH,
        "linux_backend": cfg!(target_os="linux"), "dependencies":dependencies,
        "service_installed": Path::new("/etc/systemd/system/wave-vpn-service.socket").is_file(),
        "resolver": if Path::new("/run/systemd/resolve").is_dir() { "systemd_resolved_present" } else if available("resolvconf") { "resolvconf_present" } else { "resolv_conf_or_unknown" },
        "session": context,
        "redaction": "Account names, credentials, keys, server names, addresses, preferences, environment, filesystem paths and raw logs are omitted. Failure context contains only fixed categories. No upload occurs.",
        "support_status": "development; see docs/PARITY.md for release gates"
    })
}
pub fn snapshot() -> Value {
    report(&Context::default())
}
pub fn export_report(path: &Path, report: &Value) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(&serde_json::to_vec_pretty(report)?)?;
    file.sync_all()?;
    Ok(())
}
pub fn export(path: &Path) -> Result<()> {
    export_report(path, &snapshot())
}
