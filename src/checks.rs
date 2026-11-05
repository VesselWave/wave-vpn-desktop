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
    /// Record `new` unless the provider already observed a mismatch: within one
    /// connection a failure is never downgraded to unavailable or verified.
    pub fn latch(&mut self, new: Outcome) {
        if *self != Self::Failed {
            *self = new;
        }
    }
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
    /// Apply a newer report from the same connection, keeping observed failures.
    pub fn merge(&mut self, newer: &Results) {
        self.ipv4.latch(newer.ipv4);
        self.ipv6.latch(newer.ipv6);
        self.dns.latch(newer.dns);
    }
    /// Only checks that stop a connection. An unavailable check service never
    /// does, so it must not be blamed for an unrelated disconnect.
    pub fn failure(&self) -> Option<&'static str> {
        if self.dns == Outcome::Failed {
            Some("DNS verification failed. Use Recover and retry.")
        } else if self.ipv4 == Outcome::Failed || self.ipv6 == Outcome::Failed {
            Some("VPN tunnel identity did not match. Connection stopped.")
        } else {
            None
        }
    }
    pub fn issue(&self) -> Option<&'static str> {
        if self.dns == Outcome::Failed {
            Some("DNS verification failed. Use Recover and retry.")
        } else if self.dns == Outcome::Unavailable {
            Some("AirVPN DNS verification is unavailable; the connection is not verified.")
        } else if self.ipv4 == Outcome::Failed || self.ipv6 == Outcome::Failed {
            Some("VPN tunnel identity did not match. Connection stopped.")
        } else if self.ipv4 == Outcome::Unavailable {
            Some("AirVPN IPv4 verification is unavailable; the connection is not verified.")
        } else if self.ipv6 == Outcome::Unavailable {
            Some("AirVPN IPv6 verification is unavailable; the connection is not verified.")
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

/// The network side of a check. Linux uses the provider endpoints through the
/// tunnel; tests use a fake. Nothing here decides whether to disconnect.
pub trait Probe {
    /// `/check/tun/` from the tunnel address `local`.
    fn tunnel(&mut self, local: IpAddr) -> Result<serde_json::Value>;
    /// Resolve `host` through the system resolver. The result is irrelevant:
    /// the provider's receipt proves which resolver asked.
    fn resolve(&mut self, host: &str);
    /// `/check/dns/` from the tunnel address `local`.
    fn dns(&mut self, local: IpAddr) -> Result<serde_json::Value>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Verified,
    /// Verification turned off by the user.
    Skipped,
    /// The check service did not answer. Not evidence of a leak; the
    /// connection stays up and is shown as unverified.
    Unavailable,
    /// The provider saw another identity or resolver: stop the connection.
    Failed(&'static str),
    Stopped,
}
impl Verdict {
    /// Only a provider-observed mismatch ends the session. A check-service
    /// outage must not disconnect (and later reconnect) every client at once.
    pub fn stops_connection(self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

/// One pass over the provider checks, stopping at the first non-verified step
/// so an unavailable service receives no further requests. `results` carries
/// over between passes of one connection: a failure already recorded there
/// makes the pass fail whatever the service answers now.
pub fn verify_once(
    check: &ProviderCheck,
    locals: &[IpAddr],
    probe: &mut dyn Probe,
    token: &str,
    results: &mut Results,
    publish: &mut dyn FnMut(&Results),
    stop: &dyn Fn() -> bool,
) -> Verdict {
    let verdict = if results.failure().is_some() {
        // Already failed in an earlier pass: no further requests are needed.
        Verdict::Unavailable
    } else {
        pass(check, locals, probe, token, results, publish, stop)
    };
    match (verdict, results.failure()) {
        (Verdict::Verified | Verdict::Unavailable, Some(_)) if results.dns == Outcome::Failed => {
            Verdict::Failed(DNS_FAILED)
        }
        (Verdict::Verified | Verdict::Unavailable, Some(_)) => Verdict::Failed(IDENTITY_FAILED),
        _ => verdict,
    }
}
const DNS_FAILED: &str = "System DNS verification failed; use Recover and retry";
const IDENTITY_FAILED: &str = "VPN exit identity check failed";
fn pass(
    check: &ProviderCheck,
    locals: &[IpAddr],
    probe: &mut dyn Probe,
    token: &str,
    results: &mut Results,
    publish: &mut dyn FnMut(&Results),
    stop: &dyn Fn() -> bool,
) -> Verdict {
    if check.validate().is_err() || locals.is_empty() {
        return Verdict::Unavailable;
    }
    for local in locals {
        if stop() {
            return Verdict::Stopped;
        }
        let outcome = if local.is_ipv6() {
            &mut results.ipv6
        } else {
            &mut results.ipv4
        };
        match probe.tunnel(*local) {
            Ok(response) if verify_ip(&response, *local).is_ok() => {
                outcome.latch(Outcome::Verified)
            }
            Ok(_) => {
                outcome.latch(Outcome::Failed);
                publish(results);
                return Verdict::Failed(IDENTITY_FAILED);
            }
            Err(_) => {
                outcome.latch(Outcome::Unavailable);
                publish(results);
                return Verdict::Unavailable;
            }
        }
        publish(results);
    }
    if stop() {
        return Verdict::Stopped;
    }
    probe.resolve(&check.dns_query.replace("{hash}", token));
    if stop() {
        return Verdict::Stopped;
    }
    match probe.dns(locals[0]) {
        Ok(response) if verify_dns(&response, token).is_ok() => {
            results.dns.latch(Outcome::Verified)
        }
        Ok(_) => {
            results.dns.latch(Outcome::Failed);
            publish(results);
            return Verdict::Failed(DNS_FAILED);
        }
        Err(_) => {
            results.dns.latch(Outcome::Unavailable);
            publish(results);
            return Verdict::Unavailable;
        }
    }
    publish(results);
    Verdict::Verified
}

/// Most passes made right after the first handshake, whatever the option says.
pub const MAX_ATTEMPTS: u32 = 3;
/// Pause before pass `attempt` (1-based retries): 1 s, 2 s, 4 s at most.
pub fn retry_pause(attempt: u32) -> Duration {
    Duration::from_secs(1u64 << attempt.saturating_sub(1).min(2))
}

/// Verification happens once per connection, like Eddie: a few spaced passes
/// right after the first handshake, then never again for that session.
/// Only an unavailable check service is retried. A provider-observed mismatch
/// is terminal: a later outage or success cannot erase it.
pub fn verify_at_connect(
    attempts: u64,
    once: &mut dyn FnMut() -> Verdict,
    pause: &mut dyn FnMut(Duration),
    stop: &dyn Fn() -> bool,
) -> Verdict {
    let attempts = attempts.clamp(1, u64::from(MAX_ATTEMPTS)) as u32;
    let mut last = Verdict::Unavailable;
    for attempt in 0..attempts {
        if attempt > 0 {
            pause(retry_pause(attempt));
        }
        if stop() {
            return Verdict::Stopped;
        }
        last = once();
        if last != Verdict::Unavailable {
            return last;
        }
    }
    last
}

/// The worker error for a provider-observed (or local route) mismatch. It is
/// reported as not retryable, so clients do not reconnect into the same leak.
#[derive(Debug)]
pub struct VerificationFailed(pub &'static str);
impl std::fmt::Display for VerificationFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for VerificationFailed {}

/// What the connection monitor should do on a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Due {
    Idle,
    /// Provider checks (network requests). Happens once per session.
    Verify,
    /// Local routing table check only; no network traffic.
    Routes,
}
/// Local route checks repeat; provider checks never do.
pub const ROUTE_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub struct Schedule {
    verified_at: Option<Duration>,
    routes_at: Duration,
}
impl Schedule {
    /// `elapsed` is time since the worker started; `handshake` whether one happened.
    pub fn tick(&mut self, elapsed: Duration, handshake: bool) -> Due {
        match self.verified_at {
            None if handshake => {
                self.verified_at = Some(elapsed);
                self.routes_at = elapsed;
                Due::Verify
            }
            None => Due::Idle,
            Some(_) if elapsed.saturating_sub(self.routes_at) >= ROUTE_INTERVAL => {
                self.routes_at = elapsed;
                Due::Routes
            }
            Some(_) => Due::Idle,
        }
    }
}
