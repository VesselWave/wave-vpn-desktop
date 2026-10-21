// SPDX-License-Identifier: GPL-3.0-or-later
//! Automatic account storage: prefer Secret Service; use a private plaintext
//! file only when the service is unavailable, with a warning in the UI.
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credentials {
    pub login: String,
    pub password: String,
}
impl Drop for Credentials {
    fn drop(&mut self) {
        self.login.zeroize();
        self.password.zeroize();
    }
}
#[cfg(target_os = "linux")]
fn call(action: &str, input: Option<&[u8]>) -> Result<Zeroizing<Vec<u8>>> {
    use std::{process::Command, time::Duration};
    let mut cmd = Command::new("/usr/bin/secret-tool");
    cmd.arg(action);
    if action == "store" {
        cmd.arg("--label=Wave VPN AirVPN account");
    }
    cmd.args(["application", "wave-vpn", "schema", "airvpn-account-v1"]);
    let mut result = crate::process::output(&mut cmd, input, Duration::from_secs(60))?;
    result.stderr.zeroize();
    if !result.status.success() {
        result.stdout.zeroize();
    }
    // Never include keyring stdout/stderr in user-visible errors or logs.
    ensure!(
        result.status.success()
            || matches!(action, "lookup" | "clear") && result.status.code() == Some(1),
        "Unable to access saved account. Unlock your desktop keyring and try again."
    );
    Ok(Zeroizing::new(result.stdout))
}
#[cfg(not(target_os = "linux"))]
fn call(_action: &str, _input: Option<&[u8]>) -> Result<Zeroizing<Vec<u8>>> {
    anyhow::bail!("Saved credentials currently require Linux Secret Service")
}
pub fn save(credentials: &Credentials) -> Result<()> {
    ensure!(
        !credentials.login.is_empty() && !credentials.password.is_empty(),
        "Sign in before saving credentials"
    );
    let bytes = Zeroizing::new(serde_json::to_vec(credentials)?);
    ensure!(bytes.len() <= 16384, "Saved account is too large");
    call("store", Some(&bytes))?;
    Ok(())
}
pub fn load() -> Result<Credentials> {
    let bytes = call("lookup", None)?;
    ensure!(bytes.len() <= 16384, "Saved account is too large");
    let credentials: Credentials = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Saved account is unavailable or damaged"))?;
    ensure!(
        !credentials.login.is_empty() && !credentials.password.is_empty(),
        "Saved account is empty"
    );
    Ok(credentials)
}
pub fn forget() -> Result<()> {
    call("clear", None)?;
    Ok(())
}

/// Read only Eddie's existing profile key; never change or delete its item.
pub fn eddie_password(name: &str) -> Result<Zeroizing<String>> {
    ensure!(
        !name.is_empty() && name.len() <= 255 && !name.contains(['\0', '\r', '\n']),
        "Invalid Eddie keyring identifier"
    );
    #[cfg(target_os = "linux")]
    {
        let mut command = std::process::Command::new("/usr/bin/secret-tool");
        command.args(["lookup", "Eddie Profile", name]);
        let mut result =
            crate::process::output(&mut command, None, std::time::Duration::from_secs(60))?;
        result.stderr.zeroize();
        let bytes = Zeroizing::new(std::mem::take(&mut result.stdout));
        ensure!(result.status.success(), "Unable to read Eddie profile key; unlock the system keyring or export a password-protected profile from Eddie");
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| anyhow::anyhow!("Invalid Eddie keyring value"))?;
        let password = Zeroizing::new(text.trim_end_matches(['\r', '\n']).to_owned());
        ensure!(
            !password.is_empty() && password.len() <= 4096,
            "Invalid Eddie keyring value"
        );
        Ok(password)
    }
    #[cfg(not(target_os = "linux"))]
    anyhow::bail!("Eddie keyring import requires Linux Secret Service")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    Keyring,
    Plaintext,
}
impl Storage {
    /// A locked or declined keyring remains the selected secure backend. Never
    /// silently downgrade a failed keyring operation to plaintext.
    pub fn detect() -> Self {
        #[cfg(target_os = "linux")]
        if std::path::Path::new("/usr/bin/secret-tool").is_file() {
            let mut command = std::process::Command::new("/usr/bin/gdbus");
            command.args([
                "call",
                "--session",
                "--dest",
                "org.freedesktop.DBus",
                "--object-path",
                "/org/freedesktop/DBus",
                "--method",
                "org.freedesktop.DBus.StartServiceByName",
                "org.freedesktop.secrets",
                "0",
            ]);
            if crate::process::output(&mut command, None, std::time::Duration::from_secs(10))
                .is_ok_and(|result| result.status.success())
            {
                return Self::Keyring;
            }
            let mut command = std::process::Command::new("/usr/bin/dbus-send");
            command.args([
                "--session",
                "--print-reply",
                "--dest=org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus.StartServiceByName",
                "string:org.freedesktop.secrets",
                "uint32:0",
            ]);
            if crate::process::output(&mut command, None, std::time::Duration::from_secs(10))
                .is_ok_and(|result| result.status.success())
            {
                return Self::Keyring;
            }
        }
        Self::Plaintext
    }
    pub fn load(self, directory: &std::path::Path) -> Result<Option<Credentials>> {
        if self == Self::Keyring {
            let bytes = call("lookup", None)?;
            if !bytes.is_empty() {
                ensure!(bytes.len() <= 16384, "Saved account is too large");
                return decode_credentials(&bytes).map(Some);
            }
        }
        // Migrate an existing fallback to the keyring after successful sign-in.
        load_plaintext(directory)
    }
    pub fn save(self, directory: &std::path::Path, credentials: &Credentials) -> Result<()> {
        if self == Self::Keyring {
            save(credentials)?;
            remove_plaintext(directory)?;
        } else {
            save_plaintext(directory, credentials)?;
        }
        Ok(())
    }
    pub fn forget(self, directory: &std::path::Path) -> Result<()> {
        if self == Self::Keyring {
            forget()?;
        }
        remove_plaintext(directory)
    }
}
fn decode_credentials(bytes: &[u8]) -> Result<Credentials> {
    let credentials: Credentials =
        serde_json::from_slice(bytes).map_err(|_| anyhow::anyhow!("Saved account is damaged"))?;
    ensure!(
        !credentials.login.is_empty() && !credentials.password.is_empty(),
        "Saved account is empty"
    );
    Ok(credentials)
}
fn load_plaintext(directory: &std::path::Path) -> Result<Option<Credentials>> {
    use std::io::Read;
    let path = directory.join("account.json");
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= 16384,
        "Invalid saved account file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            metadata.mode() & 0o077 == 0 && metadata.uid() == unsafe { libc::geteuid() },
            "Saved account file must be private and owned by you"
        );
    }
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(16385).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 16384, "Saved account file is too large");
    decode_credentials(&bytes).map(Some)
}
fn save_plaintext(directory: &std::path::Path, credentials: &Credentials) -> Result<()> {
    use std::io::Write;
    ensure!(
        !credentials.login.is_empty() && !credentials.password.is_empty(),
        "Sign in before saving credentials"
    );
    std::fs::create_dir_all(directory)?;
    ensure!(
        std::fs::symlink_metadata(directory)?.is_dir(),
        "Account directory must not be a symlink"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
    }
    let bytes = Zeroizing::new(serde_json::to_vec(credentials)?);
    ensure!(bytes.len() <= 16384, "Saved account is too large");
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    file.write_all(&bytes)?;
    file.as_file().sync_all()?;
    file.persist(directory.join("account.json"))?;
    Ok(())
}
fn remove_plaintext(directory: &std::path::Path) -> Result<()> {
    match std::fs::remove_file(directory.join("account.json")) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
