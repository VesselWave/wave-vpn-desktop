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
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

/// One encrypted POST. Tests substitute a fake; nothing else touches the network.
pub trait Transport: Send + Sync {
    fn post(&self, url: &reqwest::Url, form: &BTreeMap<String, String>) -> Result<Vec<u8>>;
}
impl<T: Transport + ?Sized> Transport for Arc<T> {
    fn post(&self, url: &reqwest::Url, form: &BTreeMap<String, String>) -> Result<Vec<u8>> {
        (**self).post(url, form)
    }
}

pub struct Http(reqwest::blocking::Client);
impl Http {
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
}
impl Transport for Http {
    fn post(&self, url: &reqwest::Url, form: &BTreeMap<String, String>) -> Result<Vec<u8>> {
        let response = self
            .0
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

#[derive(Debug)]
pub struct Cancelled;
impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AirVPN request cancelled")
    }
}
impl std::error::Error for Cancelled {}

type Pause = Box<dyn Fn(Duration) + Send + Sync>;

pub struct Client<T: Transport = Http> {
    transport: T,
    configurations: Vec<Bootstrap>,
    store: Option<BootstrapStore>,
    cancel: Option<Arc<AtomicBool>>,
    pause: Pause,
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
            cancel: None,
            pause: Box::new(std::thread::sleep),
        }
    }
    /// Use the latest stored manifest bootstrap first and store each newer one.
    pub fn remembering(mut self, store: BootstrapStore) -> Self {
        self.configurations = bootstrap::candidates(store.load());
        self.store = Some(store);
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
            .is_some_and(|c| c.load(Ordering::Relaxed))
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
        for (attempt, (index, url)) in bootstrap::attempts(&self.configurations)
            .into_iter()
            .enumerate()
        {
            if self.cancelled() {
                return Err(Cancelled.into());
            }
            let delay = bootstrap::failover_delay(attempt);
            if !delay.is_zero() {
                (self.pause)(delay);
                if self.cancelled() {
                    return Err(Cancelled.into());
                }
            }
            let result = (|| -> Result<String> {
                let parsed = bootstrap::endpoint(&url)?;
                let mut envelope = Envelope::new(&self.configurations[index])?;
                let form = envelope.encode(&parameters)?;
                let bytes = self.transport.post(&parsed, &form)?;
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
            match result {
                Ok(text) => return Ok(text),
                Err(e) if e.is::<AccountRejected>() => return Err(e),
                Err(_) => (),
            }
        }
        bail!("No AirVPN bootstrap server returned a valid response")
    }
    fn remember(&self, manifest: &Manifest) {
        if let (Some(store), Some(latest)) = (&self.store, &manifest.bootstrap) {
            // Best effort: a read-only config directory must not fail sign-in.
            let _ = store.save(latest);
        }
    }
    /// Server list only: the periodic refresh does not need the account document.
    pub fn manifest(&self, login: &str, password: &str) -> Result<Manifest> {
        let manifest = parse_manifest(&self.request("manifest", login, password)?)?;
        self.remember(&manifest);
        Ok(manifest)
    }
    pub fn login(&self, login: &str, password: &str) -> Result<(Account, Manifest)> {
        let account = parse_account(&self.request("user", login, password)?, login)?;
        let manifest = self.manifest(login, password)?;
        Ok((account, manifest))
    }
}
