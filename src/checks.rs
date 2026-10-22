// SPDX-License-Identifier: GPL-3.0-or-later
//! AirVPN's tunnel/DNS challenge, independently implemented from pinned Service.cs.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCheck {
    pub server: String,
    pub exits: Vec<IpAddr>,
    pub domain: String,
    pub dns_query: String,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    #[default]
    Unknown,
    Verified,
    Failed,
    Unavailable,
}
impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Not checked",
            Self::Verified => "Verified",
            Self::Failed => "Failed",
            Self::Unavailable => "Check unavailable",
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Results {
    pub ipv4: Outcome,
    pub ipv6: Outcome,
    pub dns: Outcome,
}
impl Results {
    pub fn issue(&self) -> Option<&'static str> {
        if self.dns == Outcome::Failed {
            Some("DNS verification failed. Use Recover and retry.")
        } else if self.dns == Outcome::Unavailable {
            Some("AirVPN DNS verification is unavailable. Retry when the service or connection recovers.")
        } else if self.ipv4 == Outcome::Failed || self.ipv6 == Outcome::Failed {
            Some("VPN tunnel identity did not match. Connection stopped.")
        } else if self.ipv4 == Outcome::Unavailable {
            Some("AirVPN IPv4 verification is unavailable.")
        } else if self.ipv6 == Outcome::Unavailable {
            Some("AirVPN IPv6 verification is unavailable.")
        } else {
            None
        }
    }
}
impl ProviderCheck {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.server.is_empty()
                && self.server.len() <= 64
                && self
                    .server
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-'),
            "Invalid provider check server"
        );
        // Accept only AirVPN's pinned check service, never caller-supplied URLs/scripts.
        ensure!(
            matches!(self.domain.as_str(), "airservers.org:89" | "airservers.org")
                && self.dns_query == "{hash}.checkdns.airvpn.org",
            "Unsupported AirVPN check service"
        );
        ensure!(
            !self.exits.is_empty() && self.exits.len() <= 32,
            "Missing provider exit addresses"
        );
        Ok(())
    }
    pub fn client(&self, local: IpAddr, timeout: Duration) -> Result<reqwest::blocking::Client> {
        self.validate()?;
        let url = reqwest::Url::parse(&format!(
            "https://{}_exit.{}/",
            self.server.to_lowercase(),
            self.domain
        ))?;
        let exit = self
            .exits
            .iter()
            .find(|ip| ip.is_ipv6() == local.is_ipv6())
            .context("Provider has no check endpoint for this IP family")?;
        Ok(reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .local_address(local)
            .timeout(timeout)
            .resolve(
                url.host_str().context("Missing check hostname")?,
                SocketAddr::new(*exit, url.port_or_known_default().unwrap_or(443)),
            )
            .build()?)
    }
    pub fn fetch(
        &self,
        client: &reqwest::blocking::Client,
        kind: &str,
    ) -> Result<serde_json::Value> {
        ensure!(matches!(kind, "tun" | "dns"), "Invalid check kind");
        let response = client
            .get(format!(
                "https://{}_exit.{}/check/{kind}/",
                self.server.to_lowercase(),
                self.domain
            ))
            .send()?
            .error_for_status()?;
        let mut bytes = Vec::new();
        response.take(8193).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 8192, "Provider check response too large");
        Ok(serde_json::from_slice(&bytes)?)
    }
}
pub fn verify_ip(response: &serde_json::Value, expected: IpAddr) -> Result<()> {
    let observed: IpAddr = response["ip"]
        .as_str()
        .context("Missing tunnel identity")?
        .parse()?;
    ensure!(
        observed == expected,
        "Provider observed an unexpected tunnel identity"
    );
    Ok(())
}
pub fn verify_dns(response: &serde_json::Value, token: &str) -> Result<()> {
    ensure!(
        response["dns"].as_str() == Some(token),
        "Provider did not observe the system DNS challenge"
    );
    Ok(())
}
