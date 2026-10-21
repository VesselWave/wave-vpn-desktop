// SPDX-License-Identifier: GPL-3.0-or-later
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use base64::{engine::general_purpose::STANDARD, Engine};
use rsa::{traits::PublicKeyParts, Pkcs1v15Encrypt, RsaPrivateKey};
use std::collections::BTreeMap;
use wave_protocol::*;

#[test]
fn rsa_session_and_aes_payload_interoperate_with_reference_wire_format() {
    let private = RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048).unwrap();
    let bootstrap = Bootstrap {
        urls: vec![],
        modulus: STANDARD.encode(private.n().to_bytes_be()),
        exponent: STANDARD.encode(private.e().to_bytes_be()),
    };
    let mut envelope = Envelope::new(&bootstrap).unwrap();
    let fields = envelope
        .encode(&BTreeMap::from([
            ("act".into(), "manifest".into()),
            ("password".into(), "a+&=é".into()),
        ]))
        .unwrap();
    // A reference-style peer, independent of Envelope's internal state and parser.
    fn decode_association(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
        std::str::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(|line| {
                let (k, v) = line.split_once(':').unwrap();
                (
                    String::from_utf8(STANDARD.decode(k).unwrap()).unwrap(),
                    STANDARD.decode(v).unwrap(),
                )
            })
            .collect()
    }
    let session = decode_association(
        &private
            .decrypt(Pkcs1v15Encrypt, &STANDARD.decode(&fields["s"]).unwrap())
            .unwrap(),
    );
    let key: [u8; 32] = session["key"].clone().try_into().unwrap();
    let iv: [u8; 16] = session["iv"].clone().try_into().unwrap();
    let data = cbc::Decryptor::<aes::Aes256>::new(&key.into(), &iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(&STANDARD.decode(&fields["d"]).unwrap())
        .unwrap();
    let params = decode_association(&data);
    assert_eq!(params["password"], "a+&=é".as_bytes());
    assert_eq!(params["act"], b"manifest");
    let expected = b"<manifest />";
    let reply = cbc::Encryptor::<aes::Aes256>::new(&key.into(), &iv.into())
        .encrypt_padded_vec_mut::<Pkcs7>(expected);
    assert_eq!(
        envelope.decode(&reply).unwrap(),
        std::str::from_utf8(expected).unwrap()
    );
    assert!(envelope.encode(&BTreeMap::new()).is_err());
    assert!(envelope.decode(b"invalid").is_err());
}

#[test]
fn parses_the_pinned_official_eddie_manifest() {
    let m = parse_manifest(include_str!("../protocol-data/fixtures/eddie-manifest.xml")).unwrap();
    assert!(m.servers.len() > 10);
    assert!(m
        .modes
        .iter()
        .any(|mode| mode.port == 1637 && mode.entry_index == 2));
    let s = m.servers.iter().find(|s| s.name == "Achernar").unwrap();
    assert_eq!(s.country, "ch");
    assert_eq!(s.load(), 73.0);
    assert!(s.available());
}

fn user() -> String {
    let key = STANDARD.encode([7u8; 32]);
    format!(
        r#"<user login="tester" wg_public_key="{key}"><keys><key name="phone"
        wg_private_key="{key}" wg_preshared="{key}" wg_ipv4="10.1.2.3/32" wg_ipv6="fd00::1/128"
        wg_dns_ipv4="10.1.0.1" wg_dns_ipv6="fd00::2"/></keys></user>"#
    )
}

#[test]
fn group_defaults_and_ip_families_preserve_the_advertised_entry_index() {
    let manifest = r#"<manifest auth_rsa_modulus="abc" auth_rsa_exponent="AQAB">
        <urls><url address="http://192.0.2.1"/></urls>
        <modes><mode type="wireguard" port="1637" entry_index="1"/></modes>
        <servers_groups><servers_group group="test" bw_max="2000" country_code="nl" location="City"/></servers_groups>
        <servers><server name="Test" group="test" ips_entry="192.0.2.1,192.0.2.2,2001:db8::1,2001:db8::2"/></servers>
        </manifest>"#;
    let m = parse_manifest(manifest).unwrap();
    assert_eq!(m.servers[0].country, "nl");
    let a = parse_account(&user(), "tester").unwrap();
    let p = a
        .profile(&m.servers[0], "phone", &m.modes[0], true)
        .unwrap();
    let config = p.config().unwrap();
    assert!(config.contains("Endpoint = [2001:db8::2]:1637"));
    assert!(config.contains("AllowedIPs = 0.0.0.0/0, ::/0"));
    assert!(a
        .profile(&m.servers[0], "deleted-device", &m.modes[0], false)
        .is_err());
}

#[test]
fn rejects_identity_mismatch_dtd_and_directive_injection() {
    assert!(parse_account(&user(), "other").is_err());
    assert!(validate_document(
        r#"<!DOCTYPE manifest [<!ENTITY x SYSTEM "file:///etc/passwd">]><manifest>&x;</manifest>"#,
        "manifest"
    )
    .is_err());
    let key = STANDARD.encode([7u8; 32]);
    let mut p = TunnelProfile {
        private_key: key.clone(),
        public_key: key.clone(),
        preshared_key: key,
        addresses: vec!["10.1.2.3/32\nPostUp = malicious".into()],
        dns: vec!["10.1.0.1".into()],
        endpoint: "192.0.2.1".parse().unwrap(),
        port: 1637,
    };
    assert!(p.config().is_err());
    p.addresses = vec!["10.1.2.3/32".into()];
    p.dns = vec!["10.1.0.1\nPostUp = malicious".into()];
    assert!(p.config().is_err());
}

#[test]
#[ignore = "Contacts real AirVPN bootstrap with a synthetic invalid account"]
fn live_bootstrap_decrypts_an_explicit_account_refusal() {
    let bootstrap = Bootstrap::eddie();
    let params = BTreeMap::from([
        ("act".into(), "user".into()),
        (
            "login".into(),
            "wave-protocol-test-invalid-000000000000".into(),
        ),
        ("password".into(), "intentionally-invalid".into()),
        ("software".into(), "WaveVPN_0.2-dev".into()),
        ("system".into(), "Linux".into()),
        ("arch".into(), "test".into()),
        ("version".into(), "295".into()),
    ]);
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();
    let mut verified = false;
    for url in bootstrap.urls.iter().take(2) {
        let mut envelope = Envelope::new(&bootstrap).unwrap();
        let result = client
            .post(url)
            .form(&envelope.encode(&params).unwrap())
            .send();
        let Ok(response) = result else { continue };
        let Ok(bytes) = response.bytes() else {
            continue;
        };
        let Ok(plain) = envelope.decode(&bytes) else {
            continue;
        };
        let doc = roxmltree::Document::parse(&plain).unwrap();
        let root = doc.root_element();
        assert!(
            !root.attribute("error").unwrap_or("").is_empty()
                || !root.attribute("message_action").unwrap_or("").is_empty()
        );
        verified = true;
        break;
    }
    assert!(verified, "No real bootstrap response decrypted");
}
