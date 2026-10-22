// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub enabled: Option<bool>,
    pub recovery_needed: bool,
    pub worker_active: bool,
    pub persistent: bool,
}
/// Requires an actual nftables query, never a cached preference/status file.
pub fn table_enabled(document: &serde_json::Value) -> Result<bool> {
    let entries = document["nftables"]
        .as_array()
        .context("Invalid firewall status")?;
    let table = entries
        .iter()
        .find_map(|v| v.get("table"))
        .context("Firewall table missing")?;
    ensure!(
        table["family"] == "inet"
            && table["name"] == "wave_vpn"
            && table["comment"] == "Managed by Wave VPN",
        "Firewall table ownership conflict"
    );
    let output = entries
        .iter()
        .filter_map(|v| v.get("chain"))
        .find(|c| c["table"] == "wave_vpn" && c["name"] == "output")
        .context("Firewall output chain missing")?;
    ensure!(
        output["hook"] == "output" && output["type"] == "filter" && output["prio"] == -10,
        "Unexpected firewall output chain"
    );
    match output["policy"].as_str() {
        Some("drop") => Ok(true),
        Some("accept") => Ok(false),
        _ => anyhow::bail!("Unknown firewall policy"),
    }
}
