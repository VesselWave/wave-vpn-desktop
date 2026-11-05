// SPDX-License-Identifier: GPL-3.0-or-later
//! Bootstrap selection, persistence and failover against a fake AirVPN endpoint.
//! Nothing here opens a socket.
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::SeedableRng;
use rsa::{traits::PublicKeyParts, Pkcs1v15Encrypt, RsaPrivateKey};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};
use wave_protocol::{
    bootstrap::{attempts, candidates, failover_delay, MAX_URLS},
    AccountRejected, Bootstrap, BootstrapStore, Cancelled, Client, Transport,
};

fn key(seed: u64) -> &'static RsaPrivateKey {
    static KEYS: OnceLock<Mutex<HashMap<u64, &'static RsaPrivateKey>>> = OnceLock::new();
    let mut keys = KEYS.get_or_init(Default::default).lock().unwrap();
    keys.entry(seed).or_insert_with(|| {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        Box::leak(Box::new(RsaPrivateKey::new(&mut rng, 2048).unwrap()))
    })
}
fn config(seed: u64, urls: &[&str]) -> Bootstrap {
    let private = key(seed);
    Bootstrap {
        urls: urls.iter().map(|u| u.to_string()).collect(),
        modulus: STANDARD.encode(private.n().to_bytes_be()),
        exponent: STANDARD.encode(private.e().to_bytes_be()),
    }
}

fn user_document() -> String {
    let key = STANDARD.encode([7u8; 32]);
    format!(
        r#"<user login="tester" wg_public_key="{key}"><keys><key name="laptop"
        wg_private_key="{key}" wg_preshared="{key}" wg_ipv4="10.1.2.3/32"
        wg_dns_ipv4="10.1.0.1"/></keys></user>"#
    )
}
fn manifest_document(next: Option<&Bootstrap>) -> String {
    let (urls, modulus, exponent) = match next {
        Some(b) => (
            b.urls
                .iter()
                .map(|u| format!(r#"<url address="{u}"/>"#))
                .collect::<String>(),
            b.modulus.clone(),
            b.exponent.clone(),
        ),
        None => (
            r#"<url address="ftp://203.0.113.9"/>"#.into(),
            "abc".into(),
            "AQAB".into(),
        ),
    };
    format!(
        r#"<manifest next_update="30" auth_rsa_modulus="{modulus}" auth_rsa_exponent="{exponent}"
        check_domain="airservers.org:89" check_dns_query="{{hash}}.checkdns.airvpn.org">
        <urls>{urls}</urls>
        <modes><mode type="wireguard" port="1637" entry_index="0"/></modes>
        <servers><server name="Alpha" country_code="NL" location="City" bw_max="1000"
        ips_entry="192.0.2.10" ips_exit="192.0.2.11"/></servers></manifest>"#
    )
}

#[derive(Clone)]
enum Endpoint {
    /// Answers requests encrypted for this key seed.
    Up(u64),
    Down,
    Refuses(u64),
}

/// An AirVPN-like endpoint set: decrypts with the private key it owns and
/// encrypts its reply with the session key, exactly as the real service does.
struct Fake {
    endpoints: HashMap<String, Endpoint>,
    next_bootstrap: Option<Bootstrap>,
    calls: Mutex<Vec<(String, String)>>,
    on_call: Box<dyn Fn(usize) + Send + Sync>,
}
impl Fake {
    fn new(endpoints: &[(&str, Endpoint)]) -> Self {
        Self {
            endpoints: endpoints
                .iter()
                .map(|(u, e)| (u.to_string(), e.clone()))
                .collect(),
            next_bootstrap: None,
            calls: Mutex::new(vec![]),
            on_call: Box::new(|_| ()),
        }
    }
    fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().unwrap().clone()
    }
}
fn association(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
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
impl Transport for Fake {
    fn post(&self, url: &reqwest::Url, form: &BTreeMap<String, String>) -> anyhow::Result<Vec<u8>> {
        let address = url.as_str().trim_end_matches('/').to_owned();
        let index = {
            let mut calls = self.calls.lock().unwrap();
            calls.push((address.clone(), String::new()));
            calls.len()
        };
        (self.on_call)(index);
        let endpoint = self
            .endpoints
            .get(&address)
            .cloned()
            .unwrap_or(Endpoint::Down);
        let (seed, refuse) = match endpoint {
            Endpoint::Down => anyhow::bail!("connection refused"),
            Endpoint::Up(seed) => (seed, false),
            Endpoint::Refuses(seed) => (seed, true),
        };
        // A request wrapped for another key cannot be read: the server errors.
        let session = key(seed)
            .decrypt(Pkcs1v15Encrypt, &STANDARD.decode(&form["s"])?)
            .map_err(|_| anyhow::anyhow!("HTTP 500"))?;
        let session = association(&session);
        let aes_key: [u8; 32] = session["key"].clone().try_into().unwrap();
        let iv: [u8; 16] = session["iv"].clone().try_into().unwrap();
        let data = cbc::Decryptor::<aes::Aes256>::new(&aes_key.into(), &iv.into())
            .decrypt_padded_vec_mut::<Pkcs7>(&STANDARD.decode(&form["d"])?)
            .unwrap();
        let act = String::from_utf8(association(&data)["act"].clone())?;
        self.calls.lock().unwrap()[index - 1].1 = act.clone();
        let reply = match (refuse, act.as_str()) {
            (true, _) => r#"<user error="Wrong password"/>"#.to_owned(),
            (false, "user") => user_document(),
            (false, _) => manifest_document(self.next_bootstrap.as_ref()),
        };
        Ok(
            cbc::Encryptor::<aes::Aes256>::new(&aes_key.into(), &iv.into())
                .encrypt_padded_vec_mut::<Pkcs7>(reply.as_bytes()),
        )
    }
}
fn client(fake: &Arc<Fake>, pauses: &Arc<Mutex<Vec<Duration>>>) -> Client<Arc<Fake>> {
    let pauses = pauses.clone();
    Client::with_transport(fake.clone()).pausing(move |d| pauses.lock().unwrap().push(d))
}
fn store() -> (tempfile::TempDir, BootstrapStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = BootstrapStore::in_directory(&dir.path().join("wave-vpn"));
    (dir, store)
}

#[test]
fn embedded_definition_is_valid_and_only_a_fallback() {
    let pinned = Bootstrap::eddie();
    pinned.validate().unwrap();
    assert_eq!(candidates(None), vec![pinned.clone()]);
    assert_eq!(candidates(Some(pinned.clone())), vec![pinned.clone()]);
    let latest = config(1, &["http://198.51.100.1"]);
    assert_eq!(
        candidates(Some(latest.clone())),
        vec![latest, pinned.clone()]
    );
    let mut broken = config(1, &["http://198.51.100.1"]);
    broken.modulus = "abc".into();
    assert_eq!(candidates(Some(broken)), vec![pinned]);
}

#[test]
fn validation_refuses_unsafe_or_weak_bootstrap_data() {
    let good = config(1, &["http://198.51.100.1", "https://bootstrap.example"]);
    good.validate().unwrap();
    let with = |urls: Vec<String>| Bootstrap {
        urls,
        ..good.clone()
    };
    assert!(with(vec![]).validate().is_err());
    assert!(with(vec!["http://198.51.100.1".into(); MAX_URLS + 1])
        .validate()
        .is_err());
    for url in [
        "ftp://198.51.100.1",
        "file:///etc/passwd",
        "http://user:pass@198.51.100.1",
        "http://198.51.100.1/?redirect=x",
        "http://198.51.100.1/#x",
        "http://198.51.100.1/\n",
        "not a url",
    ] {
        assert!(with(vec![url.into()]).validate().is_err(), "{url}");
    }
    let mut rng = rand::rngs::StdRng::seed_from_u64(9);
    let weak = RsaPrivateKey::new(&mut rng, 1024).unwrap();
    let weak = Bootstrap {
        modulus: STANDARD.encode(weak.n().to_bytes_be()),
        exponent: STANDARD.encode(weak.e().to_bytes_be()),
        ..good.clone()
    };
    assert!(weak.validate().is_err());
    assert!(Bootstrap {
        modulus: "!!".into(),
        ..good
    }
    .validate()
    .is_err());
}

#[test]
fn failover_order_is_latest_first_ipv6_last_without_repeats() {
    let latest = config(
        1,
        &[
            "http://[2001:db8::1]",
            "http://198.51.100.1",
            "http://shared",
        ],
    );
    let mut pinned = config(1, &["http://shared", "http://192.0.2.1"]);
    let order = attempts(&[latest.clone(), pinned.clone()]);
    assert_eq!(
        order,
        vec![
            (0, "http://198.51.100.1".to_owned()),
            (0, "http://shared".to_owned()),
            (0, "http://[2001:db8::1]".to_owned()),
            (1, "http://192.0.2.1".to_owned()),
        ]
    );
    // The same address under a rotated key is a different attempt.
    pinned = config(2, &["http://shared"]);
    assert_eq!(
        attempts(&[latest, pinned]).last(),
        Some(&(1, "http://shared".to_owned()))
    );
}

#[test]
fn failover_pauses_grow_and_stay_bounded() {
    let delays: Vec<_> = (0..8).map(failover_delay).collect();
    assert_eq!(delays[0], Duration::ZERO);
    assert_eq!(delays[1], Duration::from_millis(250));
    assert_eq!(delays[2], Duration::from_millis(500));
    assert_eq!(delays[3], Duration::from_secs(1));
    assert!(delays[4..].iter().all(|d| *d == Duration::from_secs(2)));
}

#[test]
fn first_run_uses_the_embedded_addresses_in_order_with_backoff() {
    let fake = Arc::new(Fake::new(&[]));
    let pauses = Arc::new(Mutex::new(vec![]));
    let (_dir, store) = store();
    let client = client(&fake, &pauses).remembering(store.clone());
    assert_eq!(client.configurations(), &[Bootstrap::eddie()]);
    assert!(client.login("tester", "secret").is_err());
    let tried: Vec<_> = fake.calls().into_iter().map(|(u, _)| u).collect();
    let mut expected: Vec<_> = Bootstrap::eddie()
        .urls
        .into_iter()
        .map(|u| u.trim_end_matches('/').to_owned())
        .collect();
    expected.sort_by_key(|u| u.contains('['));
    assert_eq!(tried, expected, "each address once, IPv6 last");
    let pauses = pauses.lock().unwrap();
    assert_eq!(pauses.len(), expected.len() - 1);
    assert!(pauses.windows(2).all(|w| w[0] <= w[1]));
    assert!(pauses.iter().sum::<Duration>() <= Duration::from_secs(2 * MAX_URLS as u64));
    assert!(store.load().is_none(), "nothing valid to remember");
}

#[test]
fn latest_manifest_bootstrap_is_used_persisted_and_rotated() {
    let (_dir, store) = store();
    let first = config(1, &["http://198.51.100.1", "http://198.51.100.2"]);
    store.save(&first).unwrap();
    // The service rotates key and addresses; the manifest announces them.
    let rotated = config(2, &["http://203.0.113.5"]);
    let mut fake = Fake::new(&[
        ("http://198.51.100.1", Endpoint::Down),
        ("http://198.51.100.2", Endpoint::Up(1)),
        ("http://203.0.113.5", Endpoint::Up(2)),
    ]);
    fake.next_bootstrap = Some(rotated.clone());
    let fake = Arc::new(fake);
    let pauses = Arc::new(Mutex::new(vec![]));
    let client = client(&fake, &pauses).remembering(store.clone());
    assert_eq!(client.configurations()[0], first);
    let (account, manifest) = client.login("tester", "secret").unwrap();
    assert_eq!(account.login, "tester");
    assert_eq!(manifest.servers[0].name, "Alpha");
    assert_eq!(manifest.bootstrap.as_ref(), Some(&rotated));
    assert_eq!(
        fake.calls(),
        vec![
            ("http://198.51.100.1".into(), "".into()),
            ("http://198.51.100.2".into(), "user".into()),
            ("http://198.51.100.1".into(), "".into()),
            ("http://198.51.100.2".into(), "manifest".into()),
        ],
        "embedded addresses are not contacted while the latest ones work"
    );
    assert_eq!(store.load(), Some(rotated.clone()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(store.path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "bootstrap store is private");
    }
    // The next session talks only to the rotated configuration.
    let next = client_with(&fake, &store);
    assert_eq!(next.configurations()[0], rotated);
    next.manifest("tester", "secret").unwrap();
    assert_eq!(
        fake.calls().last(),
        Some(&("http://203.0.113.5".into(), "manifest".into()))
    );
}
fn client_with(fake: &Arc<Fake>, store: &BootstrapStore) -> Client<Arc<Fake>> {
    Client::with_transport(fake.clone())
        .pausing(|_| ())
        .remembering(store.clone())
}

#[test]
fn invalid_manifest_bootstrap_keeps_servers_and_previous_configuration() {
    let (_dir, store) = store();
    let first = config(1, &["http://198.51.100.1"]);
    store.save(&first).unwrap();
    let fake = Arc::new(Fake::new(&[("http://198.51.100.1", Endpoint::Up(1))]));
    let manifest = client_with(&fake, &store)
        .manifest("tester", "secret")
        .unwrap();
    assert!(manifest.bootstrap.is_none());
    assert_eq!(manifest.servers.len(), 1, "server list remains usable");
    assert_eq!(store.load(), Some(first));
}

#[test]
fn unreachable_latest_configuration_falls_back_to_embedded() {
    let (_dir, store) = store();
    store
        .save(&config(1, &["http://198.51.100.1", "http://198.51.100.2"]))
        .unwrap();
    let fake = Arc::new(Fake::new(&[]));
    assert!(client_with(&fake, &store)
        .manifest("tester", "secret")
        .is_err());
    let tried: Vec<_> = fake.calls().into_iter().map(|(u, _)| u).collect();
    assert_eq!(&tried[..2], ["http://198.51.100.1", "http://198.51.100.2"]);
    let pinned = Bootstrap::eddie();
    assert_eq!(tried.len(), 2 + pinned.urls.len());
    assert!(pinned
        .urls
        .iter()
        .all(|u| tried.contains(&u.trim_end_matches('/').to_owned())));
}

#[test]
fn account_refusal_stops_failover_immediately() {
    let (_dir, store) = store();
    store
        .save(&config(1, &["http://198.51.100.1", "http://198.51.100.2"]))
        .unwrap();
    let fake = Arc::new(Fake::new(&[
        ("http://198.51.100.1", Endpoint::Refuses(1)),
        ("http://198.51.100.2", Endpoint::Up(1)),
    ]));
    let error = client_with(&fake, &store)
        .login("tester", "wrong")
        .err()
        .unwrap();
    assert!(error.is::<AccountRejected>());
    assert_eq!(fake.calls().len(), 1);
}

#[test]
fn periodic_refresh_requests_only_the_manifest() {
    let (_dir, store) = store();
    store.save(&config(1, &["http://198.51.100.1"])).unwrap();
    let fake = Arc::new(Fake::new(&[("http://198.51.100.1", Endpoint::Up(1))]));
    client_with(&fake, &store)
        .manifest("tester", "secret")
        .unwrap();
    assert_eq!(
        fake.calls(),
        vec![("http://198.51.100.1".into(), "manifest".into())]
    );
}

#[test]
fn cancellation_stops_failover_between_attempts() {
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let mut fake = Fake::new(&[]);
    fake.on_call = Box::new(move |n| {
        if n == 2 {
            flag.store(true, Ordering::Relaxed);
        }
    });
    let fake = Arc::new(fake);
    let error = Client::with_transport(fake.clone())
        .pausing(|_| ())
        .cancellable(cancel)
        .login("tester", "secret")
        .err()
        .unwrap();
    assert!(error.is::<Cancelled>());
    assert_eq!(fake.calls().len(), 2);
}

#[test]
fn store_ignores_damaged_files_and_does_not_rewrite_unchanged_data() {
    let (dir, store) = store();
    assert!(store.load().is_none());
    let good = config(1, &["http://198.51.100.1"]);
    assert!(store
        .save(&Bootstrap {
            urls: vec![],
            ..good.clone()
        })
        .is_err());
    store.save(&good).unwrap();
    assert_eq!(store.load(), Some(good.clone()));
    for damaged in [
        b"{".to_vec(),
        br#"{"version":99,"bootstrap":{"urls":[],"modulus":"","exponent":""}}"#.to_vec(),
        vec![b' '; 70 * 1024],
    ] {
        std::fs::write(store.path(), damaged).unwrap();
        assert!(store.load().is_none());
    }
    store.save(&good).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // Saving replaces the file by rename, so an unchanged inode proves no rewrite.
        let inode = std::fs::metadata(store.path()).unwrap().ino();
        store.save(&good).unwrap();
        assert_eq!(std::fs::metadata(store.path()).unwrap().ino(), inode);
        let changed = config(1, &["http://198.51.100.7"]);
        store.save(&changed).unwrap();
        assert_eq!(store.load(), Some(changed));
        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink(store.path(), &link).unwrap();
        assert!(
            BootstrapStore::new(link).load().is_none(),
            "symlinks are refused"
        );
    }
}
