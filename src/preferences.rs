// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, io::Write, path::PathBuf};
use wave_protocol::options::Options;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Target {
    #[default]
    Best,
    Country(String),
    Server(String),
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub options: Options,
    pub favorites: BTreeSet<String>,
    pub target: Target,
    pub device: String,
    pub last_server: String,
}
pub fn directory() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
        .join("wave-vpn")
}
impl Preferences {
    pub fn load() -> Self {
        let mut prefs: Self = std::fs::read(directory().join("preferences.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        prefs.options = prefs.options.desktop();
        prefs
    }
    pub fn save(&self) -> Result<()> {
        self.options.validate()?;
        let dir = directory();
        std::fs::create_dir_all(&dir)?;
        anyhow::ensure!(
            std::fs::symlink_metadata(&dir)?.is_dir(),
            "Preferences directory must not be a symlink"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        }
        let mut temp = tempfile::NamedTempFile::new_in(&dir)?;
        temp.write_all(&serde_json::to_vec_pretty(self)?)?;
        temp.as_file().sync_all()?;
        temp.persist(dir.join("preferences.json"))
            .context("Unable to save preferences")?;
        Ok(())
    }
}

/// Refuse symlinks and permissive preexisting files rather than append secrets to
/// an attacker-selected path. Refuse unbounded growth; the user can rotate it.
pub fn append_log(path: &std::path::Path, line: &str) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path)?;
    let meta = file.metadata()?;
    anyhow::ensure!(
        meta.is_file() && meta.len() < 5 * 1024 * 1024,
        "Log must be a regular file smaller than 5 MiB; rotate it manually"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        anyhow::ensure!(
            meta.mode() & 0o077 == 0 && meta.uid() == unsafe { libc::geteuid() },
            "Log file must be private and owned by this user"
        );
    }
    writeln!(file, "{line}")?;
    Ok(())
}
