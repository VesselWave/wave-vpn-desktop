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
