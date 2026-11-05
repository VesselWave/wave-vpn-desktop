// SPDX-License-Identifier: GPL-3.0-or-later
// Request flow reference: AirVPN/Eddie Service.FetchUrls; see docs/PROVENANCE.md.
use crate::{
    bootstrap::{self, Bootstrap, BootstrapStore},
    parse_account, parse_manifest, validate_document, Account, AccountRejected, Envelope, Manifest,
    MAX_DOCUMENT,
};
use anyhow::{bail, Result};
use std::{
    collections::BTreeMap,
    io::Read,
    net::{IpAddr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

/// One encrypted POST. Tests substitute a fake; nothing else touches the network.
pub trait Transport: Send + Sync {
    fn post(&self, url: &reqwest::Url, form: &BTreeMap<String, String>) -> Result<Vec<u8>>;
    /// POST to a hostname endpoint pinned to `addresses`, resolved earlier, so
    /// no DNS lookup is needed (Network Lock blocks DNS outside the tunnel).
    fn post_pinned(
        &self,
        url: &reqwest::Url,
        addresses: &[SocketAddr],
        form: &BTreeMap<String, String>,
    ) -> Result<Vec<u8>> {
        let _ = addresses;
        self.post(url, form)
    }
}
impl<T: Transport + ?Sized> Transport for Arc<T> {
    fn post(&self, url: &reqwest::Url, form: &BTreeMap<String, String>) -> Result<Vec<u8>> {
        (**self).post(url, form)
    }
    fn post_pinned(
        &self,
        url: &reqwest::Url,
        addresses: &[SocketAddr],
        form: &BTreeMap<String, String>,
    ) -> Result<Vec<u8>> {
        (**self).post_pinned(url, addresses, form)
    }
}

pub struct Http {
    client: reqwest::blocking::Client,
    timeout: Duration,
}
impl Http {
    fn builder(timeout: Duration) -> reqwest::blocking::ClientBuilder {
        reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(timeout)
            .timeout(timeout)
    }
    pub fn with_timeout(seconds: u64) -> Result<Self> {
        let timeout = Duration::from_secs(seconds.clamp(1, 3600));
        Ok(Self {
            client: Self::builder(timeout).build()?,
            timeout,
        })
    }
    fn send(
        client: &reqwest::blocking::Client,
        url: &reqwest::Url,
        form: &BTreeMap<String, String>,
    ) -> Result<Vec<u8>> {
        let response = client
            .post(url.clone())
            .form(form)
            .send()?
            .error_for_status()?;
        let mut bytes = Vec::new();
        response
            .take((MAX_DOCUMENT + 1) as u64)
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}
impl Transport for Http {
    fn post(&self, url: &reqwest::Url, form: &BTreeMap<String, String>) -> Result<Vec<u8>> {
        Self::send(&self.client, url, form)
    }
    fn post_pinned(
        &self,
        url: &reqwest::Url,
        addresses: &[SocketAddr],
        form: &BTreeMap<String, String>,
    ) -> Result<Vec<u8>> {
        let host = url.host_str().unwrap_or_default();
        let client = Self::builder(self.timeout)
            .resolve_to_addrs(host, addresses)
            .build()?;
        Self::send(&client, url, form)
    }
}

#[derive(Debug)]
pub struct Cancelled;
impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AirVPN request cancelled")
    }
}
impl std::error::Error for Cancelled {}

/// Cancel the requests that share `flag`, fenced against bootstrap writes:
/// a write already past its cancellation check finishes first, and none of
/// those requests can write after this returns. Use instead of a plain store.
pub fn cancel(flag: &AtomicBool) {
    let _fence = bootstrap::persistence_lock();
    flag.store(true, Ordering::SeqCst);
}

type Pause = Box<dyn Fn(Duration) + Send + Sync>;
type Lookup = Box<dyn Fn(&str) -> Option<Vec<IpAddr>> + Send + Sync>;

pub struct Client<T: Transport = Http> {
    transport: T,
    configurations: Vec<Bootstrap>,
    store: Option<BootstrapStore>,
    resolved: bootstrap::Resolved,
    /// The stored latest configuration, whose exemption selection defines the
    /// only addresses a hostname may be pinned to.
    pin_source: Option<Bootstrap>,
    require_pins: bool,
    cancel: Option<Arc<AtomicBool>>,
    pause: Pause,
    lookup: Lookup,
}
impl Client<Http> {
    pub fn new() -> Result<Self> {
        Self::with_timeout(10)
    }
    pub fn with_timeout(seconds: u64) -> Result<Self> {
        Ok(Self::with_transport(Http::with_timeout(seconds)?))
    }
}
impl<T: Transport> Client<T> {
    pub fn with_transport(transport: T) -> Self {
        Self {
            transport,
            configurations: bootstrap::candidates(None),
            store: None,
            resolved: bootstrap::Resolved::new(),
            pin_source: None,
            require_pins: false,
            cancel: None,
            pause: Box::new(std::thread::sleep),
            lookup: Box::new(bootstrap::system_lookup),
        }
    }
    /// Use the latest stored manifest bootstrap first and store each newer one.
    pub fn remembering(mut self, store: BootstrapStore) -> Self {
        let latest = store.latest();
        self.configurations = bootstrap::candidates(latest.bootstrap.clone());
        self.pin_source = latest.bootstrap;
        self.resolved = latest.resolved;
        self.store = Some(store);
        self
    }
    /// Replace the hostname resolver used for new bootstrap hostnames; tests
    /// use a table instead of DNS.
    pub fn resolving(
        mut self,
        lookup: impl Fn(&str) -> Option<Vec<IpAddr>> + Send + Sync + 'static,
    ) -> Self {
        self.lookup = Box::new(lookup);
        self
    }
    /// Under Network Lock outside the tunnel only pinned (exempted) addresses
    /// are reachable: never fall back to ordinary DNS for a hostname endpoint
    /// without a pin; skip it instead.
    pub fn requiring_pins(mut self, require: bool) -> Self {
        self.require_pins = require;
        self
    }
    /// Stop between failover attempts once the flag is set.
    pub fn cancellable(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancel = Some(flag);
        self
    }
    /// Replace the failover pause; tests record it instead of sleeping.
    pub fn pausing(mut self, pause: impl Fn(Duration) + Send + Sync + 'static) -> Self {
        self.pause = Box::new(pause);
        self
    }
    pub fn configurations(&self) -> &[Bootstrap] {
        &self.configurations
    }
    fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::SeqCst))
    }
    pub fn request(&self, action: &str, login: &str, password: &str) -> Result<String> {
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
        let mut attempt = 0;
        for (index, url) in bootstrap::attempts(&self.configurations) {
            if self.cancelled() {
                return Err(Cancelled.into());
            }
            let Ok(parsed) = bootstrap::endpoint(&url) else {
                continue;
            };
            let pins = self
                .pin_source
                .as_ref()
                .and_then(|b| b.pinned(&parsed, &self.resolved));
            if pins.is_none() && self.require_pins && bootstrap::is_hostname(&parsed) {
                continue;
            }
            let delay = bootstrap::failover_delay(attempt);
            attempt += 1;
            if !delay.is_zero() {
                (self.pause)(delay);
                if self.cancelled() {
                    return Err(Cancelled.into());
                }
            }
            let result = (|| -> Result<String> {
                let mut envelope = Envelope::new(&self.configurations[index])?;
                let form = envelope.encode(&parameters)?;
                let bytes = match &pins {
                    Some(addresses) => self.transport.post_pinned(&parsed, addresses, &form)?,
                    None => self.transport.post(&parsed, &form)?,
                };
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
            // Blocking HTTP cannot be interrupted, so a response may arrive
            // after cancellation; it is discarded, never accepted or stored.
            if self.cancelled() {
                return Err(Cancelled.into());
            }
            match result {
                Ok(text) => return Ok(text),
                Err(e) if e.is::<AccountRejected>() => return Err(e),
                Err(_) => (),
            }
        }
        bail!("No AirVPN bootstrap server returned a valid response")
    }
    /// Store the manifest's bootstrap, fenced by cancellation and by manifest
    /// time. Only cancellation fails the request; other storage errors are
    /// best effort, so a read-only config directory does not fail sign-in.
    fn remember(&self, manifest: &Manifest, base: u64) -> Result<()> {
        let (Some(store), Some(latest)) = (&self.store, &manifest.bootstrap) else {
            return Ok(());
        };
        if self.cancelled() {
            return Err(Cancelled.into());
        }
        let previous = store.latest().resolved;
        let start = std::time::Instant::now();
        let resolved = bootstrap::resolve_hosts_bounded(latest, &previous, &*self.lookup, &|| {
            !self.cancelled() && start.elapsed() < bootstrap::RESOLVE_BUDGET
        });
        match store.save_fenced(latest, manifest.issued, &resolved, base, &|| {
            !self.cancelled()
        }) {
            Err(e) if e.is::<Cancelled>() => Err(e),
            _ => Ok(()),
        }
    }
    /// Server list only: the periodic refresh does not need the account document.
    pub fn manifest(&self, login: &str, password: &str) -> Result<Manifest> {
        // Captured before the request: a later write by another session wins ties.
        let base = self.store.as_ref().map_or(0, |s| s.revision());
        let manifest = parse_manifest(&self.request("manifest", login, password)?)?;
        self.remember(&manifest, base)?;
        Ok(manifest)
    }
    pub fn login(&self, login: &str, password: &str) -> Result<(Account, Manifest)> {
        let account = parse_account(&self.request("user", login, password)?, login)?;
        let manifest = self.manifest(login, password)?;
        Ok((account, manifest))
    }
}
