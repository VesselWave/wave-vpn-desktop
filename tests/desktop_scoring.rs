// SPDX-License-Identifier: GPL-3.0-or-later
use wave_protocol::{parse_manifest, Manifest, Server};

fn fixture(load: u64, users: u32) -> (Manifest, Server) {
    let manifest =
        parse_manifest(include_str!("../protocol-data/fixtures/eddie-manifest.xml")).unwrap();
    let mut server = manifest.servers[0].clone();
    server.capacity = 1000;
    server.bandwidth = load * 625_000;
    server.users = users;
    server.users_max = 100;
    server.score_base = 0.;
    server.warning_open.clear();
    server.warning_closed.clear();
    (manifest, server)
}

#[test]
fn nearby_low_load_servers_outrank_distant_congested_servers() {
    let (manifest, local) = fixture(21, 362);
    let (_, distant) = fixture(74, 150);
    assert_eq!(manifest.desktop_score(&local, Some(27), 0), Some(48));
    assert_eq!(manifest.desktop_score(&distant, Some(129), 0), Some(399));
    // User count reflects capacity too; a larger server must not be penalized
    // enough to erase its load and latency advantage.
    assert!(
        manifest.desktop_score(&local, Some(27), 0)
            < manifest.desktop_score(&distant, Some(129), 0)
    );
}

#[test]
fn measured_latency_and_heavy_congestion_change_ranking() {
    let (manifest, server) = fixture(60, 100);
    assert_eq!(manifest.desktop_score(&server, None, 0), Some(60));
    assert_eq!(manifest.desktop_score(&server, Some(20), 0), Some(80));
    let (_, congested) = fixture(80, 100);
    assert_eq!(manifest.desktop_score(&congested, Some(20), 0), Some(500));
    assert_eq!(manifest.desktop_score(&server, Some(20), 10), Some(70));
}

#[test]
fn warnings_and_closed_servers_have_no_displayable_score() {
    let (manifest, mut server) = fixture(21, 100);
    server.warning_open = "Maintenance".into();
    assert_eq!(manifest.desktop_score(&server, Some(20), 0), None);
    server.warning_open.clear();
    server.capacity = 0;
    assert_eq!(manifest.desktop_score(&server, Some(20), 0), None);
}
