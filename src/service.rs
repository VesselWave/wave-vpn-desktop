// SPDX-License-Identifier: GPL-3.0-or-later
//! Root-owned, socket-activated networking broker. Peer UID authorization and a
//! fixed action allowlist reuse the existing validated worker protocol.
use anyhow::Result;
use std::{path::Path, process::Command};

/// Whether the networking service is installed, so an action will not prompt.
pub fn available() -> bool {
    #[cfg(target_os = "linux")]
    {
        Path::new(linux::SOCKET).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}
/// The desktop UID authorized at service installation, from the root-only
/// service configuration. Network Lock limits bootstrap exemptions to it.
pub fn authorized_uid() -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        linux::authorized_uid()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
pub fn command(executable: &Path, action: &str) -> Result<Command> {
    command_cancellable(executable, action, || false)
}
pub fn command_cancellable(
    executable: &Path,
    action: &str,
    cancelled: impl Fn() -> bool,
) -> Result<Command> {
    anyhow::ensure!(!cancelled(), "Networking action cancelled");
    #[cfg(target_os = "linux")]
    {
        if !Path::new(linux::SOCKET).exists() {
            // The helper is copied from this executable, which must be root-owned:
            // a binary run straight from a download folder can never qualify.
            anyhow::ensure!(
                linux::trusted(executable).is_ok(),
                "Wave VPN isn't installed yet. Install the package for your system, or run \
                 ./install.sh from the downloaded folder, then open Wave VPN from your app menu."
            );
            let mut setup = Command::new("/usr/bin/pkexec")
                .arg(executable)
                .arg("service-install")
                .spawn()?;
            let start = std::time::Instant::now();
            let status = loop {
                if let Some(status) = setup.try_wait()? {
                    break status;
                }
                if cancelled() || start.elapsed() >= std::time::Duration::from_secs(120) {
                    let _ = setup.kill();
                    let _ = setup.wait();
                    anyhow::bail!(
                        "Networking service setup cancelled or timed out; retry Connect when ready"
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            };
            anyhow::ensure!(
                status.success(),
                "Networking service setup was cancelled or failed"
            );
        }
        anyhow::ensure!(!cancelled(), "Networking action cancelled");
        let mut command = Command::new(executable);
        command.args(["service-client", action]);
        Ok(command)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (executable, action);
        anyhow::bail!("VPN networking currently requires Linux")
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use anyhow::{ensure, Context, Result};
    use serde::{Deserialize, Serialize};
    use std::{
        fs::{self, OpenOptions},
        io::{BufRead, BufReader, Read, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{
                fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
                net::{UnixListener, UnixStream},
            },
        },
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::Duration,
    };
    pub const SOCKET: &str = "/run/wave-vpn-service.sock";
    const CONFIG: &str = "/etc/wave-vpn/service.json";
    const HELPER: &str = "/usr/local/libexec/wave-vpn-helper";
    const SOCKET_UNIT: &str = "/etc/systemd/system/wave-vpn-service.socket";
    const SERVICE_UNIT: &str = "/etc/systemd/system/wave-vpn-service.service";
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Config {
        uid: u32,
    }
    fn root() -> Result<()> {
        ensure!(
            unsafe { libc::geteuid() } == 0,
            "Administrator privileges required"
        );
        Ok(())
    }
    pub(super) fn trusted(path: &Path) -> Result<PathBuf> {
        let path = path.canonicalize()?;
        for parent in path.ancestors() {
            let metadata = fs::metadata(parent)?;
            ensure!(
                metadata.uid() == 0 && metadata.mode() & 0o022 == 0,
                "Service files must be root-owned and not writable by other users"
            );
        }
        Ok(path)
    }
    fn config() -> Result<Config> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(CONFIG)?;
        let metadata = file.metadata()?;
        ensure!(
            metadata.is_file()
                && metadata.uid() == 0
                && metadata.mode() & 0o077 == 0
                && metadata.len() <= 1024,
            "Unsafe service authorization file"
        );
        let mut bytes = Vec::new();
        file.take(1025).read_to_end(&mut bytes)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
    pub(super) fn authorized_uid() -> Option<u32> {
        config().ok().map(|c| c.uid).filter(|uid| *uid > 0)
    }
    fn atomic(path: &str, bytes: &[u8], mode: u32) -> Result<()> {
        let path = Path::new(path);
        let parent = trusted(path.parent().context("Missing parent")?)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(mode))?;
        file.write_all(bytes)?;
        file.as_file().sync_all()?;
        file.persist(path)?;
        Ok(())
    }
    fn systemctl(args: &[&str]) -> Result<()> {
        let executable = trusted(Path::new("/usr/bin/systemctl"))?;
        let mut command = Command::new(executable);
        command.args(args);
        let output = crate::process::output(&mut command, None, Duration::from_secs(40))?;
        ensure!(output.status.success(), "Service manager operation failed");
        Ok(())
    }
    pub fn install(uid: Option<u32>) -> Result<()> {
        root()?;
        ensure!(
            Path::new("/run/systemd/system").is_dir(),
            "Networking service requires systemd"
        );
        ensure!(
            !Path::new("/run/wave-vpn/recovery.json").exists(),
            "Disconnect the VPN before updating the networking service"
        );
        let uid = uid
            .or_else(|| {
                std::env::var("SUDO_UID")
                    .or_else(|_| std::env::var("PKEXEC_UID"))
                    .ok()
                    .and_then(|value| value.parse().ok())
            })
            .or_else(|| config().ok().map(|value| value.uid))
            .context("Run with sudo as your desktop user, or supply --uid")?;
        ensure!(uid > 0, "Authorize a desktop user, not root");
        let source = trusted(&std::env::current_exe()?)?;
        ensure!(source.is_file(), "Invalid service executable");
        for directory in ["/etc/wave-vpn", "/usr/local/libexec"] {
            fs::create_dir_all(directory)?;
            trusted(Path::new(directory))?;
        }
        fs::set_permissions("/etc/wave-vpn", fs::Permissions::from_mode(0o700))?;
        if Path::new(SOCKET_UNIT).exists() {
            systemctl(&[
                "stop",
                "wave-vpn-service.socket",
                "wave-vpn-service.service",
            ])?;
        }
        atomic(HELPER, &fs::read(source)?, 0o755)?;
        atomic(CONFIG, &serde_json::to_vec(&Config { uid })?, 0o600)?;
        atomic(SOCKET_UNIT, b"[Unit]\nDescription=Wave VPN networking socket\n[Socket]\nListenStream=/run/wave-vpn-service.sock\nSocketMode=0666\nRemoveOnStop=true\n[Install]\nWantedBy=sockets.target\n", 0o644)?;
        atomic(SERVICE_UNIT, b"[Unit]\nDescription=Wave VPN networking service\nRequires=wave-vpn-service.socket\nAfter=wave-vpn-service.socket\n[Service]\nExecStart=/usr/local/libexec/wave-vpn-helper service-serve\nUser=root\nUMask=0077\nNoNewPrivileges=true\nProtectHome=true\nPrivateTmp=true\nKillMode=control-group\nTimeoutStopSec=30\nRestart=on-failure\n", 0o644)?;
        systemctl(&["daemon-reload"])?;
        systemctl(&["enable", "--now", "wave-vpn-service.socket"])?;
        println!("Networking service installed. Future VPN actions need no password.");
        Ok(())
    }
    pub fn uninstall() -> Result<()> {
        root()?;
        ensure!(
            !Path::new("/run/wave-vpn/recovery.json").exists(),
            "Disconnect and recover the VPN before removing the service"
        );
        systemctl(&["disable", "--now", "wave-vpn-service.socket"])?;
        systemctl(&["stop", "wave-vpn-service.service"])?;
        for path in [SOCKET_UNIT, SERVICE_UNIT, HELPER, CONFIG] {
            match fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                _ => {}
            }
        }
        systemctl(&["daemon-reload"])?;
        Ok(())
    }
    pub(crate) fn peer_uid(stream: &UnixStream) -> Result<u32> {
        let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut length,
            )
        };
        ensure!(
            result == 0 && length as usize == std::mem::size_of::<libc::ucred>(),
            "Unable to identify service peer"
        );
        Ok(credentials.uid)
    }
    fn allowed(action: &str) -> bool {
        matches!(
            action,
            "transport-worker"
                | "recover"
                | "lock-enable"
                | "lock-disable"
                | "lock-update"
                | "network-status"
        )
    }
    fn finish(stream: &mut UnixStream, success: bool) -> Result<()> {
        writeln!(
            stream,
            "{}",
            serde_json::json!({"event":"service-exit", "success":success})
        )?;
        Ok(())
    }
    // Keep frames flowing while the controller pipe remains open for commands.
    // An explicit bounded read/write loop avoids Linux splice waiting for EOF
    // when io::copy bridges a long-lived pipe and a Unix socket.
    fn relay(reader: &mut impl Read, writer: &mut impl Write) -> std::io::Result<()> {
        let mut buffer = [0u8; 8192];
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                return Ok(());
            }
            writer.write_all(&buffer[..count])?;
            writer.flush()?;
        }
    }
    fn handle(mut stream: UnixStream, helper: &Path) -> Result<()> {
        let uid = peer_uid(&stream)?;
        ensure!(
            uid == 0 || uid == config()?.uid,
            "This user is not authorized for Wave VPN networking"
        );
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let mut action = Vec::new();
        loop {
            let mut byte = [0];
            stream.read_exact(&mut byte)?;
            if byte[0] == b'\n' {
                break;
            }
            action.push(byte[0]);
            ensure!(action.len() <= 32, "Service action too long");
        }
        let action = std::str::from_utf8(&action)?;
        ensure!(allowed(action), "Unsupported service action");
        stream.set_read_timeout(None)?;
        let mut child = Command::new(helper)
            .arg(action)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut input = stream.try_clone()?;
        let mut stdin = child.stdin.take().context("Worker stdin missing")?;
        let input_thread = std::thread::spawn(move || {
            let _ = relay(&mut input, &mut stdin);
        });
        let output = Arc::new(Mutex::new(stream.try_clone()?));
        let forward = |reader: Box<dyn Read + Send>, output: Arc<Mutex<UnixStream>>| {
            std::thread::spawn(move || {
                let mut reader = BufReader::new(reader);
                loop {
                    let mut line = Vec::new();
                    match reader.by_ref().take(65537).read_until(b'\n', &mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) if line.len() > 65536 => break,
                        Ok(_) => {
                            if output.lock().unwrap().write_all(&line).is_err() {
                                break;
                            }
                        }
                    }
                }
            })
        };
        let stdout = forward(
            Box::new(child.stdout.take().context("Worker stdout missing")?),
            output.clone(),
        );
        let stderr = forward(
            Box::new(child.stderr.take().context("Worker stderr missing")?),
            output,
        );
        let status = child.wait()?;
        let _ = stream.shutdown(std::net::Shutdown::Read);
        let _ = input_thread.join();
        let _ = stdout.join();
        let _ = stderr.join();
        finish(&mut stream, status.success())?;
        Ok(())
    }
    pub fn serve() -> Result<()> {
        root()?;
        config()?;
        let helper = trusted(&std::env::current_exe()?)?;
        ensure!(
            std::env::var("LISTEN_PID")?.parse::<u32>()? == std::process::id()
                && std::env::var("LISTEN_FDS")? == "1",
            "Start this service through its systemd socket"
        );
        let listener = unsafe { UnixListener::from_raw_fd(3) };
        // Do not leak the listening socket into any networking child.
        ensure!(
            unsafe { libc::fcntl(3, libc::F_SETFD, libc::FD_CLOEXEC) } == 0,
            "Unable to protect listening socket"
        );
        let active = Arc::new(AtomicUsize::new(0));
        for connection in listener.incoming() {
            let mut stream = connection?;
            // Bound even rejection writes before processing untrusted peers.
            stream.set_write_timeout(Some(Duration::from_secs(1)))?;
            // Authenticate peer UID before reserving bounded worker slots.
            // Reject unauthorized or unidentifiable peers promptly without
            // consuming bounded slots for legitimate desktop clients.
            let uid = match peer_uid(&stream) {
                Ok(uid) => uid,
                Err(_) => {
                    let _ = writeln!(
                        stream,
                        "Networking service rejected or could not complete this request"
                    );
                    let _ = finish(&mut stream, false);
                    continue;
                }
            };
            // Read current authorization on every admission; a revoked UID must
            // not retain access to slots until the broker restarts. handle()
            // checks again before starting a worker.
            let authorized = uid == 0 || config().is_ok_and(|current| uid == current.uid);
            if !authorized {
                let _ = writeln!(
                    stream,
                    "This user is not authorized for Wave VPN networking"
                );
                let _ = finish(&mut stream, false);
                continue;
            }
            if active.fetch_add(1, Ordering::AcqRel) >= 8 {
                active.fetch_sub(1, Ordering::AcqRel);
                let _ = writeln!(
                    stream,
                    "Networking service rejected or could not complete this request"
                );
                let _ = finish(&mut stream, false);
                continue;
            }
            let active = active.clone();
            let helper = helper.clone();
            std::thread::spawn(move || {
                if stream
                    .try_clone()
                    .map_err(anyhow::Error::from)
                    .and_then(|stream| handle(stream, &helper))
                    .is_err()
                {
                    let _ = writeln!(
                        stream,
                        "Networking service rejected or could not complete this request"
                    );
                    let _ = finish(&mut stream, false);
                }
                active.fetch_sub(1, Ordering::AcqRel);
            });
        }
        Ok(())
    }
    pub fn client(action: &str) -> Result<()> {
        ensure!(allowed(action), "Unsupported service action");
        let metadata = fs::symlink_metadata(SOCKET)?;
        ensure!(
            metadata.file_type().is_socket() && metadata.uid() == 0,
            "Unsafe networking service socket"
        );
        let mut stream = UnixStream::connect(SOCKET)?;
        ensure!(
            peer_uid(&stream)? == 0,
            "Networking service is not running as root"
        );
        writeln!(stream, "{action}")?;
        let mut input = stream.try_clone()?;
        std::thread::spawn(move || {
            let _ = relay(&mut std::io::stdin().lock(), &mut input);
            let _ = input.shutdown(std::net::Shutdown::Write);
        });
        for line in BufReader::new(stream).lines() {
            let line = line?;
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                if value["event"] == "service-exit" {
                    ensure!(
                        value["success"] == true,
                        "Networking action failed; inspect Logs"
                    );
                    return Ok(());
                }
            }
            println!("{line}");
            std::io::stdout().flush()?;
        }
        anyhow::bail!("Networking service connection was lost")
    }
}
#[cfg(target_os = "linux")]
pub use linux::{client, install, serve, uninstall};
#[cfg(not(target_os = "linux"))]
pub fn install(_uid: Option<u32>) -> Result<()> {
    anyhow::bail!("Networking service requires Linux")
}
#[cfg(not(target_os = "linux"))]
pub fn uninstall() -> Result<()> {
    anyhow::bail!("Networking service requires Linux")
}
#[cfg(not(target_os = "linux"))]
pub fn serve() -> Result<()> {
    anyhow::bail!("Networking service requires Linux")
}
#[cfg(not(target_os = "linux"))]
pub fn client(_action: &str) -> Result<()> {
    anyhow::bail!("Networking service requires Linux")
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::linux::*;
    use std::os::unix::net::UnixStream;

    #[test]
    fn peer_uid_identifies_local_socket_pair() {
        let (s1, _s2) = UnixStream::pair().unwrap();
        let uid = peer_uid(&s1).unwrap();
        assert_eq!(uid, unsafe { libc::geteuid() });
    }
}
