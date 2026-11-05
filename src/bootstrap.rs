// SPDX-License-Identifier: GPL-3.0-or-later
//! Bootstrap endpoints and RSA key. Eddie reads them from the latest manifest
//! and keeps the provider definition only as a default; see docs/PROVENANCE.md.
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use rsa::{traits::PublicKeyParts, BigUint, RsaPublicKey};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

/// More addresses than this in a manifest is malformed, not a longer failover.
pub const MAX_URLS: usize = 16;
const MAX_URL_LENGTH: usize = 256;
const MAX_STORED: u64 = 64 * 1024;
const STORE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bootstrap {
    pub urls: Vec<String>,
    pub modulus: String,
    pub exponent: String,
}
impl Bootstrap {
    /// The pinned Eddie provider definition compiled into the binary.
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
    pub fn public_key(&self) -> Result<RsaPublicKey> {
        let key = RsaPublicKey::new(
            BigUint::from_bytes_be(&STANDARD.decode(&self.modulus)?),
            BigUint::from_bytes_be(&STANDARD.decode(&self.exponent)?),
        )?;
        Ok(key)
    }
    /// A usable bootstrap has 1..=16 plain HTTP(S) endpoints and an RSA key of
    /// at least 2048 bits. Anything else keeps the previous configuration.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.urls.is_empty() && self.urls.len() <= MAX_URLS,
            "Bootstrap must list 1 to {MAX_URLS} addresses"
        );
        for url in &self.urls {
            endpoint(url)?;
        }
        ensure!(
            self.public_key()?.size() * 8 >= 2048,
            "Bootstrap RSA key is too short"
        );
        Ok(())
    }
}

/// Parse one bootstrap address; credentials, queries and fragments are refused.
pub fn endpoint(url: &str) -> Result<reqwest::Url> {
    ensure!(
        url.len() <= MAX_URL_LENGTH && !url.chars().any(char::is_control),
        "Invalid bootstrap URL"
    );
    let parsed = reqwest::Url::parse(url).context("Invalid bootstrap URL")?;
    ensure!(
        matches!(parsed.scheme(), "http" | "https")
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.host_str().is_some_and(|h| !h.is_empty())
            && parsed.query().is_none()
            && parsed.fragment().is_none(),
        "Invalid bootstrap URL"
    );
    Ok(parsed)
}

/// Configurations to try, latest manifest first, pinned definition last.
pub fn candidates(latest: Option<Bootstrap>) -> Vec<Bootstrap> {
    let pinned = Bootstrap::eddie();
    match latest {
        Some(latest) if latest != pinned && latest.validate().is_ok() => vec![latest, pinned],
        _ => vec![pinned],
    }
}

/// Every distinct (configuration, address) pair once, in failover order.
/// Within a configuration IPv6 literals go last, as before; an address already
/// tried with the same key is not repeated.
pub fn attempts(configurations: &[Bootstrap]) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for (index, configuration) in configurations.iter().enumerate() {
        let mut urls = configuration.urls.clone();
        urls.sort_by_key(|u| u.contains('['));
        for url in urls {
            let repeated = out.iter().any(|(i, u)| {
                *u == url
                    && configurations[*i].modulus == configuration.modulus
                    && configurations[*i].exponent == configuration.exponent
            });
            if !repeated {
                out.push((index, url));
            }
        }
    }
    out
}

/// Pause before failover attempt `attempt` (0 is the first, immediate attempt):
/// 250 ms doubling to at most 2 s, so an outage is not hammered in a burst.
pub fn failover_delay(attempt: usize) -> Duration {
    if attempt == 0 {
        Duration::ZERO
    } else {
        Duration::from_millis(250u64 << (attempt - 1).min(3))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    bootstrap: Bootstrap,
}

/// The latest validated manifest bootstrap, kept in the user's config directory.
/// It holds only public endpoints and a public key; no account data.
#[derive(Clone, Debug)]
pub struct BootstrapStore {
    path: PathBuf,
}
impl BootstrapStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn in_directory(directory: &Path) -> Self {
        Self::new(directory.join("bootstrap.json"))
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Missing, oversized, symlinked, malformed or invalid files read as none,
    /// so the embedded definition still works on first run or after damage.
    pub fn load(&self) -> Option<Bootstrap> {
        let meta = std::fs::symlink_metadata(&self.path).ok()?;
        if !meta.is_file() || meta.len() > MAX_STORED {
            return None;
        }
        let mut bytes = Vec::new();
        std::fs::File::open(&self.path)
            .ok()?
            .take(MAX_STORED + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        let stored: Stored = serde_json::from_slice(&bytes).ok()?;
        (stored.version == STORE_VERSION && stored.bootstrap.validate().is_ok())
            .then_some(stored.bootstrap)
    }
    /// Validate, then replace atomically. Unchanged data is not rewritten.
    pub fn save(&self, bootstrap: &Bootstrap) -> Result<()> {
        bootstrap.validate()?;
        if self.load().as_ref() == Some(bootstrap) {
            return Ok(());
        }
        let directory = self
            .path
            .parent()
            .context("Bootstrap store has no directory")?;
        std::fs::create_dir_all(directory)?;
        ensure!(
            std::fs::symlink_metadata(directory)?.is_dir(),
            "Bootstrap directory must not be a symlink"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
        }
        let mut temp = tempfile::NamedTempFile::new_in(directory)?;
        temp.write_all(&serde_json::to_vec_pretty(&Stored {
            version: STORE_VERSION,
            bootstrap: bootstrap.clone(),
        })?)?;
        temp.as_file().sync_all()?;
        temp.persist(&self.path)
            .context("Unable to save bootstrap configuration")?;
        Ok(())
    }
}
