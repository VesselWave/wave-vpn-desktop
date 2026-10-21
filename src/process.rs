// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded subprocess I/O and lifetime; no shell interpolation.
use anyhow::{ensure, Context, Result};
use std::{
    io::{Read, Write},
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
const LIMIT: u64 = 1024 * 1024;
pub fn output(command: &mut Command, input: Option<&[u8]>, timeout: Duration) -> Result<Output> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().context("Missing child stdout")?;
    let stderr = child.stderr.take().context("Missing child stderr")?;
    let read = |stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || -> std::io::Result<Vec<u8>> {
            let mut bytes = Vec::new();
            stream.take(LIMIT + 1).read_to_end(&mut bytes)?;
            Ok(bytes)
        })
    };
    let out = read(Box::new(stdout));
    let err = read(Box::new(stderr));
    let writer = input.map(|bytes| {
        let bytes = Zeroizing::new(bytes.to_vec());
        let mut pipe = child.stdin.take().expect("Child stdin is piped");
        std::thread::spawn(move || pipe.write_all(&bytes))
    });
    let start = Instant::now();
    let mut expired = false;
    let mut status = None;
    let status = loop {
        if status.is_none() {
            status = child.try_wait()?;
        }
        if out.is_finished() && err.is_finished() && writer.as_ref().is_none_or(|w| w.is_finished())
        {
            if let Some(status) = status {
                break status;
            }
        }
        if start.elapsed() >= timeout {
            expired = true;
            #[cfg(unix)]
            {
                // Child was started in its own group; also close inherited pipes
                // in subprocesses before joining I/O threads.
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
            }
            let _ = child.kill();
            break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = out
        .join()
        .map_err(|_| anyhow::anyhow!("Child output reader failed"))??;
    let stderr = err
        .join()
        .map_err(|_| anyhow::anyhow!("Child error reader failed"))??;
    ensure!(!expired, "System command timed out");
    if let Some(writer) = writer {
        writer
            .join()
            .map_err(|_| anyhow::anyhow!("Child input writer failed"))??;
    }
    ensure!(
        stdout.len() as u64 <= LIMIT && stderr.len() as u64 <= LIMIT,
        "System command output exceeded limit"
    );
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}
