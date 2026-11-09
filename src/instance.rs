// SPDX-License-Identifier: GPL-3.0-or-later
//! One Wave VPN window per desktop user (U2404 B-19). Two GUIs ran two
//! controllers: one saw the other's worker as a raw lock error and quitting
//! the owner tore the tunnel down under the other. The first instance holds
//! a lock in the user's runtime directory and listens on a socket beside it;
//! a later launch asks it to show its window and exits.
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    time::Duration,
};

pub enum Instance {
    /// This process is the only GUI; serve show requests with `Primary::serve`.
    Primary(Primary),
    /// Another GUI runs. `shown`: it confirmed showing its window.
    Existing { shown: bool },
}
pub struct Primary {
    _lock: File,
    listener: UnixListener,
}

/// The per-user directory: `$XDG_RUNTIME_DIR` (private to the user), else
/// the private configuration directory. Never a shared /tmp.
pub fn directory() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_dir())
        .map(|p| p.join("wave-vpn"))
        .unwrap_or_else(wave_protocol::config_directory)
}

pub fn claim(dir: &Path) -> std::io::Result<Instance> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(dir.join("gui.lock"))?;
    let socket = dir.join("gui.sock");
    if fs2::FileExt::try_lock_exclusive(&lock).is_ok() {
        // Holding the lock proves a leftover socket is stale.
        let _ = std::fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket)?;
        return Ok(Instance::Primary(Primary {
            _lock: lock,
            listener,
        }));
    }
    Ok(Instance::Existing {
        shown: ask_to_show(&socket).unwrap_or(false),
    })
}

fn ask_to_show(socket: &Path) -> std::io::Result<bool> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    stream.write_all(b"show\n")?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    Ok(reply == "ok\n")
}

impl Primary {
    /// Answer show requests for the life of the process.
    pub fn serve(self, show: impl Fn() + Send + 'static) {
        std::thread::spawn(move || {
            let _lock = self._lock;
            for stream in self.listener.incoming().map_while(Result::ok) {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                let mut line = String::new();
                let Ok(mut reader) = stream.try_clone().map(BufReader::new) else {
                    continue;
                };
                if reader.read_line(&mut line).is_ok() && line == "show\n" {
                    show();
                    let _ = (&stream).write_all(b"ok\n");
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn forum_u2404_b19_second_launch_shows_the_first_window() {
        let dir = tempfile::tempdir().unwrap();
        let Instance::Primary(primary) = claim(dir.path()).unwrap() else {
            panic!("first launch must be the primary instance");
        };
        let shown = Arc::new(AtomicUsize::new(0));
        let counter = shown.clone();
        primary.serve(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        for _ in 0..2 {
            assert!(matches!(
                claim(dir.path()).unwrap(),
                Instance::Existing { shown: true }
            ));
        }
        assert_eq!(shown.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn forum_u2404_b19_stale_socket_after_a_crash_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        // A crashed GUI leaves its socket file but no lock holder.
        drop(UnixListener::bind(dir.path().join("gui.sock")).unwrap());
        assert!(matches!(claim(dir.path()).unwrap(), Instance::Primary(_)));
    }
}
