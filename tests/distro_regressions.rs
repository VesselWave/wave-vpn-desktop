// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline regression contracts for defects found in the distribution test
//! rounds (docs/qa/DISTRO-UBUNTU-2404.md, B-ids). GUI cases live in
//! tests/support/forum_gui_cases.rs, package hook cases in
//! scripts/tests/test_packaging_scripts.py. `scripts/check-forum.py` runs
//! every `forum_` test and reports the named contracts.
use std::path::Path;

// B-01: a package upgrade replaces the helper while a connection may run. The
// broker detects that its executable was replaced and restarts once idle.
#[test]
fn forum_u2404_b01_replaced_helper_executable_is_detected() {
    use wave_protocol::service::executable_replaced;
    assert!(executable_replaced(Path::new(
        "/usr/local/libexec/wave-vpn-helper (deleted)"
    )));
    assert!(!executable_replaced(Path::new(
        "/usr/local/libexec/wave-vpn-helper"
    )));
}

// B-10: both programs report their version; the GUI rejects unknown
// arguments and exits cleanly without a display instead of panicking.
// Killed after 20 s, so a GUI that ignores its arguments and opens a window
// fails the test instead of hanging it.
fn run(binary: &str, args: &[&str]) -> std::process::Output {
    use std::process::{Command, Stdio};
    let mut child = Command::new(binary)
        .args(args)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env_remove("DISPLAY")
        .env_remove("WAVE_PREVIEW")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > std::time::Duration::from_secs(20) {
            let _ = child.kill();
            panic!("{binary} {args:?} did not exit");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    child.wait_with_output().unwrap()
}

#[test]
fn forum_u2404_b10_cli_and_gui_report_their_version() {
    let version = env!("CARGO_PKG_VERSION");
    for (binary, name) in [
        (env!("CARGO_BIN_EXE_wave-vpn"), "wave-vpn"),
        (env!("CARGO_BIN_EXE_wave-vpn-gui"), "wave-vpn-gui"),
    ] {
        for flag in ["--version", "-V"] {
            let output = run(binary, &[flag]);
            assert!(output.status.success(), "{name} {flag}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                format!("{name} {version}")
            );
        }
    }
}

#[test]
fn forum_u2404_b10_gui_rejects_unknown_arguments() {
    let output = run(env!("CARGO_BIN_EXE_wave-vpn-gui"), &["--bogus"]);
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("--bogus") && error.contains("Usage"),
        "{error}"
    );
    assert!(!error.contains("panicked"), "{error}");
}

#[cfg(target_os = "linux")]
#[test]
fn forum_u2404_b10_gui_without_a_display_exits_cleanly() {
    let output = run(env!("CARGO_BIN_EXE_wave-vpn-gui"), &[]);
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("display"), "{error}");
    assert!(!error.contains("panicked"), "{error}");
}
