// SPDX-License-Identifier: GPL-3.0-or-later
#[cfg(unix)]
#[test]
fn bounded_commands_capture_both_streams_and_terminate_process_groups() {
    use std::{
        process::Command,
        time::{Duration, Instant},
    };
    use wave_protocol::process::output;
    let result = output(
        Command::new("/bin/sh").args(["-c", "cat; printf error >&2"]),
        Some(b"synthetic-input"),
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout, b"synthetic-input");
    assert_eq!(result.stderr, b"error");
    let start = Instant::now();
    let result = output(
        Command::new("/bin/sh").args(["-c", "sleep 10 &"]),
        None,
        Duration::from_millis(100),
    );
    assert!(result.is_err());
    assert!(start.elapsed() < Duration::from_secs(3));
}
