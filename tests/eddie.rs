// SPDX-License-Identifier: GPL-3.0-or-later
use wave_protocol::eddie::decode;
#[test]
fn imports_independent_openssl_python_vectors_and_rejects_tampering() {
    // Synthetic vectors generated with Python hashlib/hmac + OpenSSL enc.
    // No archived application/test code or real credentials are included.
    for bytes in [
        include_bytes!("../protocol-data/fixtures/eddie-import/v1n-sha1.bin").as_slice(),
        include_bytes!("../protocol-data/fixtures/eddie-import/v1p-sha1.bin").as_slice(),
        include_bytes!("../protocol-data/fixtures/eddie-import/v2n-sha1.bin").as_slice(),
        include_bytes!("../protocol-data/fixtures/eddie-import/v2p-sha1.bin").as_slice(),
        include_bytes!("../protocol-data/fixtures/eddie-import/v1n-sha256.bin").as_slice(),
        include_bytes!("../protocol-data/fixtures/eddie-import/v1p-sha256.bin").as_slice(),
        include_bytes!("../protocol-data/fixtures/eddie-import/v2n-sha256.bin").as_slice(),
        include_bytes!("../protocol-data/fixtures/eddie-import/v2p-sha256.bin").as_slice(),
    ] {
        let result = decode(bytes, "profile-test-password").unwrap();
        assert_eq!(result.login, "synthetic-user");
        assert_eq!(result.password, "synthetic-pass");
        assert_eq!(result.options.get("mtu"), "1320");
        assert_eq!(result.ignored, 2);
        let mut changed = bytes.to_vec();
        let end = changed.len() - 1;
        changed[end] ^= 1;
        assert!(decode(&changed, "profile-test-password").is_err());
        for length in [0, 1, 3, 66, 80, bytes.len() - 1] {
            assert!(decode(&bytes[..length], "profile-test-password").is_err());
        }
        if bytes[2] == b'p' {
            assert!(decode(bytes, "wrong-password").is_err());
        }
    }
}
#[test]
fn rejects_unsafe_xml_and_invalid_supported_values() {
    for xml in [
        "<!DOCTYPE eddie [<!ENTITY x 'secret'>]><eddie><options><option name='login' value='&x;'/></options></eddie>",
        "<eddie><options><option name='login' value='a'/><option name='login' value='b'/></options></eddie>",
        "<eddie><options><option name='servers.startlast' value='maybe'/></options></eddie>",
        "<user><options/></user>",
    ] { assert!(decode(xml.as_bytes(), "").is_err()); }
    assert!(decode(b"v2s", "").is_err());
}

#[test]
fn advanced_eddie_options_are_omitted_even_when_wave_does_not_support_their_values() {
    let imported = decode(b"<eddie><options><option name='network.ipv4.mode' value='in-out'/><option name='dns.mode' value='none'/><option name='servers.startlast' value='true'/></options></eddie>", "").unwrap();
    assert_eq!(imported.ignored, 2);
    assert!(imported.options.on("startlast"));
    assert_eq!(imported.options.get("ipv4"), "in");
    assert_eq!(imported.options.get("dns_mode"), "auto");
}
