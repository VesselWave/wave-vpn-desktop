// SPDX-License-Identifier: GPL-3.0-or-later
// Independently written against pinned Eddie references; see docs/PROVENANCE.md.
pub mod bootstrap;
pub mod checks;
mod client;
pub mod credentials;
pub mod diagnostics;
mod documents;
pub mod eddie;
mod envelope;
pub mod firewall;
pub mod latency;
pub mod lifecycle;
pub mod options;
pub mod process;
pub mod protection;
pub mod resolver;
pub mod schedule;
pub mod service;
pub mod transport;

use anyhow::{Context, Result};
pub use bootstrap::{Bootstrap, BootstrapStore};
pub use client::{
    cancel as cancel_requests, Cancelled, Client, Http, Transport, SIGN_IN_DEADLINE,
    SIGN_IN_TIMED_OUT,
};
pub use documents::*;
pub use envelope::{association, Envelope};

/// Per-user configuration directory shared by the desktop app and the CLI.
pub fn config_directory() -> std::path::PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
        .join("wave-vpn")
}

pub const MAX_DOCUMENT: usize = 4 * 1024 * 1024;
pub fn validate_document(text: &str, kind: &str) -> Result<()> {
    anyhow::ensure!(text.len() <= MAX_DOCUMENT, "Document too large");
    let doc = roxmltree::Document::parse(text).context("Malformed AirVPN XML")?;
    let root = doc.root_element();
    anyhow::ensure!(root.tag_name().name() == kind, "Unexpected AirVPN document");
    anyhow::ensure!(
        root.attribute("error").unwrap_or("").is_empty()
            && root.attribute("message_action").unwrap_or("").is_empty(),
        "AirVPN rejected this request"
    );
    Ok(())
}

#[derive(Debug)]
pub struct AccountRejected;
impl std::fmt::Display for AccountRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "AirVPN rejected the account request. Check your credentials and account status.",
        )
    }
}
impl std::error::Error for AccountRejected {}
#[derive(Debug)]
pub struct AccountDevicesMissing;
impl std::fmt::Display for AccountDevicesMissing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Your AirVPN account has no WireGuard devices. Add a device in the AirVPN client area, then sign in again.")
    }
}
impl std::error::Error for AccountDevicesMissing {}
pub fn sign_in_error(error: &anyhow::Error) -> String {
    if error.is::<AccountRejected>() || error.is::<AccountDevicesMissing>() {
        error.to_string()
    } else {
        "Cannot reach or validate AirVPN sign-in. Use Recover in Overview, then retry; it keeps your Network Lock setting. If Eddie left DNS or firewall settings behind, recover them in Eddie.".into()
    }
}
