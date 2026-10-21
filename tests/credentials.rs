// SPDX-License-Identifier: GPL-3.0-or-later
#[test]
#[ignore = "Use scripts/test-keyring.sh in a private D-Bus session with synthetic credentials"]
fn isolated_secret_service_round_trip_and_delete() {
    use wave_protocol::credentials::{self, Credentials};
    assert_eq!(
        std::env::var("WAVE_ISOLATED_KEYRING_TEST").as_deref(),
        Ok("1")
    );
    credentials::save(&Credentials {
        login: "synthetic-user".into(),
        password: "synthetic-password".into(),
    })
    .unwrap();
    let account = credentials::load().unwrap();
    assert_eq!(account.login, "synthetic-user");
    assert_eq!(account.password, "synthetic-password");
    credentials::forget().unwrap();
    assert!(credentials::load().is_err());
    use credentials::Storage;
    assert_eq!(Storage::detect(), Storage::Keyring);
    let dir = tempfile::tempdir().unwrap();
    let account = Credentials {
        login: "synthetic-user".into(),
        password: "synthetic-password".into(),
    };
    Storage::Plaintext.save(dir.path(), &account).unwrap();
    let restored = Storage::Keyring.load(dir.path()).unwrap().unwrap();
    assert_eq!(restored.login, account.login);
    Storage::Keyring.save(dir.path(), &restored).unwrap();
    assert!(!dir.path().join("account.json").exists());
    assert!(Storage::Keyring.load(dir.path()).unwrap().is_some());
    Storage::Keyring.forget(dir.path()).unwrap();
    assert!(Storage::Keyring.load(dir.path()).unwrap().is_none());
}

#[test]
#[ignore = "Use scripts/test-keyring.sh; synthetic profiles on a private bus"]
fn eddie_keyring_lookup_uses_profile_id_and_legacy_filename_without_modifying_items() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    assert_eq!(
        std::env::var("WAVE_ISOLATED_KEYRING_TEST").as_deref(),
        Ok("1")
    );
    let dir = tempfile::tempdir().unwrap();
    for fixture in [
        include_bytes!("../protocol-data/fixtures/eddie-import/v2p-sha256.bin").as_slice(),
        include_bytes!("../protocol-data/fixtures/eddie-import/v1p-sha1.bin").as_slice(),
    ] {
        let mut bytes = fixture.to_vec();
        bytes[2] = b's';
        let path = dir.path().join("legacy.profile");
        let name = if bytes[1] == b'2' {
            std::str::from_utf8(&bytes[3..67]).unwrap()
        } else {
            "legacy.profile"
        };
        let mut child = Command::new("/usr/bin/secret-tool")
            .args([
                "store",
                "--label=Wave isolated Eddie fixture",
                "Eddie Profile",
                name,
            ])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"profile-test-password\n")
            .unwrap();
        assert!(child.wait().unwrap().success());
        std::fs::write(&path, &bytes).unwrap();
        let before = std::fs::read(&path).unwrap();
        let imported = wave_protocol::eddie::read(&path, "").unwrap();
        assert_eq!(imported.login, "synthetic-user");
        assert_eq!(imported.password, "synthetic-pass");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(
            &**wave_protocol::credentials::eddie_password(name).unwrap(),
            "profile-test-password"
        );
        assert!(Command::new("/usr/bin/secret-tool")
            .args(["clear", "Eddie Profile", name])
            .status()
            .unwrap()
            .success());
        assert!(wave_protocol::eddie::read(&path, "").is_err());
    }
}

#[test]
#[ignore = "Set WAVE_EDDIE_TEST_PROFILE to a synthetic profile saved by installed Eddie"]
fn imports_profile_written_by_installed_eddie() {
    let path = std::env::var_os("WAVE_EDDIE_TEST_PROFILE").expect("synthetic profile path");
    let imported = wave_protocol::eddie::read(std::path::Path::new(&path), "").unwrap();
    assert_eq!(imported.login, "synthetic-user");
    assert_eq!(imported.password, "synthetic-pass");
}

#[test]
fn plaintext_fallback_is_private_restorable_and_removed_on_sign_out() {
    use wave_protocol::credentials::{Credentials, Storage};
    let dir = tempfile::tempdir().unwrap();
    assert!(Storage::Plaintext.load(dir.path()).unwrap().is_none());
    let account = Credentials {
        login: "synthetic-user".into(),
        password: "synthetic-password".into(),
    };
    Storage::Plaintext.save(dir.path(), &account).unwrap();
    let restored = Storage::Plaintext.load(dir.path()).unwrap().unwrap();
    assert_eq!(restored.login, account.login);
    assert_eq!(restored.password, account.password);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(dir.path().join("account.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    Storage::Plaintext.forget(dir.path()).unwrap();
    assert!(Storage::Plaintext.load(dir.path()).unwrap().is_none());
    Storage::Plaintext.forget(dir.path()).unwrap();
}

#[test]
#[cfg(unix)]
fn plaintext_restore_rejects_symlinks_and_shared_permissions() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    use wave_protocol::credentials::{Credentials, Storage};
    let dir = tempfile::tempdir().unwrap();
    let account = Credentials {
        login: "synthetic-user".into(),
        password: "synthetic-password".into(),
    };
    Storage::Plaintext.save(dir.path(), &account).unwrap();
    let path = dir.path().join("account.json");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Storage::Plaintext.load(dir.path()).is_err());
    std::fs::rename(&path, dir.path().join("other")).unwrap();
    symlink(dir.path().join("other"), &path).unwrap();
    assert!(Storage::Plaintext.load(dir.path()).is_err());
    Storage::Plaintext.forget(dir.path()).unwrap();
    assert!(dir.path().join("other").exists());
}
