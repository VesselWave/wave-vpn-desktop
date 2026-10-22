// SPDX-License-Identifier: GPL-3.0-or-later
// Independently written against pinned Eddie references; see docs/PROVENANCE.md.
pub mod checks;
pub mod credentials;
pub mod diagnostics;
mod documents;
pub mod eddie;
mod envelope;
pub mod firewall;
pub mod lifecycle;
pub mod options;
pub mod process;
pub mod protection;
pub mod service;
pub mod transport;

use anyhow::{bail, Context, Result};
pub use documents::*;
pub use envelope::{association, Envelope};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Read, time::Duration};

#[derive(Clone, Deserialize, Serialize)]
pub struct Bootstrap {
    pub urls: Vec<String>,
    pub modulus: String,
    pub exponent: String,
}
impl Bootstrap {
    pub fn eddie() -> Self {
        let definition: serde_json::Value =
            serde_json::from_str(include_str!("../protocol-data/AirVPN.json"))
                .expect("pinned Eddie provider definition");
        let m = &definition["manifest"];
        Self {
            urls: m["urls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["address"].as_str().unwrap().to_owned())
                .collect(),
            modulus: m["auth_rsa_modulus"].as_str().unwrap().to_owned(),
            exponent: m["auth_rsa_exponent"].as_str().unwrap().to_owned(),
        }
    }
}

pub struct Client(reqwest::blocking::Client);
impl Client {
    pub fn new() -> Result<Self> {
        Self::with_timeout(10)
    }
    pub fn with_timeout(seconds: u64) -> Result<Self> {
        Ok(Self(
            reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .connect_timeout(Duration::from_secs(seconds.clamp(1, 3600)))
                .timeout(Duration::from_secs(seconds.clamp(1, 3600)))
                .build()?,
        ))
    }
    pub fn request(
        &self,
        bootstrap: &Bootstrap,
        action: &str,
        login: &str,
        password: &str,
    ) -> Result<String> {
        anyhow::ensure!(
            matches!(action, "user" | "manifest"),
            "Unsupported AirVPN request"
        );
        let parameters = BTreeMap::from([
            ("act".into(), action.into()),
            ("login".into(), login.into()),
            ("password".into(), password.into()),
            ("software".into(), "WaveVPN_0.1-dev".into()),
            ("system".into(), "Linux".into()),
            ("arch".into(), std::env::consts::ARCH.into()),
            ("version".into(), "295".into()),
        ]);
        let mut urls = bootstrap.urls.clone();
        urls.sort_by_key(|u| u.contains('['));
        for url in urls {
            let attempt = (|| -> Result<String> {
                let parsed = reqwest::Url::parse(&url)?;
                anyhow::ensure!(
                    matches!(parsed.scheme(), "http" | "https")
                        && parsed.username().is_empty()
                        && parsed.password().is_none(),
                    "Invalid bootstrap URL"
                );
                let mut envelope = Envelope::new(bootstrap)?;
                let form = envelope.encode(&parameters)?;
                let response = self.0.post(parsed).form(&form).send()?.error_for_status()?;
                let mut bytes = Vec::new();
                response
                    .take((MAX_DOCUMENT + 1) as u64)
                    .read_to_end(&mut bytes)?;
                anyhow::ensure!(bytes.len() <= MAX_DOCUMENT, "AirVPN response too large");
                let plain = envelope.decode(&bytes)?;
                // A valid encrypted refusal is an account error, not an unreachable endpoint.
                let doc = roxmltree::Document::parse(&plain)?;
                if doc
                    .root_element()
                    .attribute("error")
                    .is_some_and(|s| !s.is_empty())
                    || doc
                        .root_element()
                        .attribute("message_action")
                        .is_some_and(|s| !s.is_empty())
                {
                    return Err(AccountRejected.into());
                }
                validate_document(&plain, action)?;
                Ok(plain)
            })();
            match attempt {
                Ok(text) => return Ok(text),
                Err(e) if e.is::<AccountRejected>() => return Err(e),
                Err(_) => (),
            }
        }
        bail!("No AirVPN bootstrap server returned a valid response")
    }
    pub fn login(&self, login: &str, password: &str) -> Result<(Account, Manifest)> {
        let bootstrap = Bootstrap::eddie();
        let account = parse_account(&self.request(&bootstrap, "user", login, password)?, login)?;
        let manifest = parse_manifest(&self.request(&bootstrap, "manifest", login, password)?)?;
        Ok((account, manifest))
    }
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
