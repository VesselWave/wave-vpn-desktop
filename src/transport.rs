// SPDX-License-Identifier: GPL-3.0-or-later
// Linux transport uses AirVPN's official Hummingbird 2.x, not the archived helper.
use crate::TunnelProfile;
use anyhow::{ensure, Context, Result};
use std::{
    io::{BufRead, Write},
    process::{Command, Stdio},
};

/// Executed by pkexec. Only accepts validated structured WireGuard data over stdin.
/// EOF or "stop" terminates Hummingbird gracefully so it can restore networking.
pub fn worker() -> Result<()> {
    ensure!(
        nix::unistd::Uid::effective().is_root(),
        "The transport worker requires administrator privileges"
    );
    let input = std::io::stdin();
    let mut reader = input.lock();
    let mut bytes = Vec::new();
    std::io::Read::take(&mut reader, 32 * 1024).read_until(b'\n', &mut bytes)?;
    let profile: TunnelProfile = serde_json::from_slice(&bytes)?;
    use zeroize::Zeroize;
    bytes.zeroize();
    let mut config = tempfile::Builder::new()
        .prefix("wave-")
        .suffix(".conf")
        .tempfile_in("/run")?;
    let mut text = profile.config()?;
    config.write_all(text.as_bytes())?;
    config.flush()?;
    text.zeroize();
    let binary = ["/usr/local/bin/hummingbird", "/usr/bin/hummingbird"]
        .iter()
        .find(|p| std::path::Path::new(p).is_file())
        .context("Install AirVPN Suite 2.x (Hummingbird) first")?;
    let binary = std::fs::canonicalize(binary)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        for component in binary.ancestors() {
            let meta = std::fs::metadata(component)?;
            ensure!(
                meta.uid() == 0 && meta.mode() & 0o022 == 0,
                "Hummingbird and its parent directories must be root-owned and not writable by group/others"
            );
        }
    }
    // No shell, no PATH lookup, no operator-controlled arguments or scripts.
    let mut child = Command::new(binary)
        .arg(config.path())
        .args(["--network-lock", "on"])
        .stdin(Stdio::null())
        .spawn()?;
    let pid = nix::unistd::Pid::from_raw(child.id() as i32);
    let mut command = String::new();
    // Keep reading on a thread so an early Hummingbird exit doesn't leave a worker waiting forever.
    std::thread::spawn(move || {
        let _ = std::io::stdin().read_line(&mut command);
        let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM);
    });
    drop(reader);
    let status = child.wait()?;
    ensure!(
        status.success(),
        "Hummingbird exited with {status}; inspect networking before reconnecting"
    );
    Ok(())
}
