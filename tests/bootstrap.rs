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
    bootstrap::{
        attempts, candidates, failover_delay, resolve_hosts, validate_exemptions, Exemption,
        Resolved, Saved, MAX_ADDRESSES_PER_HOST, MAX_EXEMPTIONS, MAX_URLS,
    },
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
    manifest_document_at(next, None)
}
fn manifest_document_at(next: Option<&Bootstrap>, time: Option<u64>) -> String {
    let time = time.map(|t| format!(r#" time="{t}""#)).unwrap_or_default();
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
            r#"<url address="ftp://45.90.113.9"/>"#.into(),
            "abc".into(),
            "AQAB".into(),
        ),
    };
    format!(
        r#"<manifest{time} next_update="30" auth_rsa_modulus="{modulus}" auth_rsa_exponent="{exponent}"
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
    manifest_time: Option<u64>,
    calls: Mutex<Vec<(String, String)>>,
    pinned: Mutex<Vec<(String, Vec<std::net::SocketAddr>)>>,
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
            manifest_time: None,
            calls: Mutex::new(vec![]),
            pinned: Mutex::new(vec![]),
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
            (false, _) => manifest_document_at(self.next_bootstrap.as_ref(), self.manifest_time),
        };
        Ok(
            cbc::Encryptor::<aes::Aes256>::new(&aes_key.into(), &iv.into())
                .encrypt_padded_vec_mut::<Pkcs7>(reply.as_bytes()),
        )
    }
    fn post_pinned(
        &self,
        url: &reqwest::Url,
        addresses: &[std::net::SocketAddr],
        form: &BTreeMap<String, String>,
    ) -> anyhow::Result<Vec<u8>> {
        self.pinned.lock().unwrap().push((
            url.as_str().trim_end_matches('/').to_owned(),
            addresses.to_vec(),
        ));
        self.post(url, form)
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
    let latest = config(1, &["http://45.90.100.1"]);
    assert_eq!(
        candidates(Some(latest.clone())),
        vec![latest, pinned.clone()]
    );
    let mut broken = config(1, &["http://45.90.100.1"]);
    broken.modulus = "abc".into();
    assert_eq!(candidates(Some(broken)), vec![pinned]);
}

#[test]
fn validation_refuses_unsafe_or_weak_bootstrap_data() {
    let good = config(1, &["http://45.90.100.1", "https://bootstrap.example"]);
    good.validate().unwrap();
    let with = |urls: Vec<String>| Bootstrap {
        urls,
        ..good.clone()
    };
    assert!(with(vec![]).validate().is_err());
    assert!(with(vec!["http://45.90.100.1".into(); MAX_URLS + 1])
        .validate()
        .is_err());
    for url in [
        "ftp://45.90.100.1",
        "file:///etc/passwd",
        "http://user:pass@45.90.100.1",
        "http://45.90.100.1/?redirect=x",
        "http://45.90.100.1/#x",
        "http://45.90.100.1/\n",
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
            "http://[2a01:4f8::1]",
            "http://45.90.100.1",
            "http://shared",
        ],
    );
    let mut pinned = config(1, &["http://shared", "http://192.0.2.1"]);
    let order = attempts(&[latest.clone(), pinned.clone()]);
    assert_eq!(
        order,
        vec![
            (0, "http://45.90.100.1".to_owned()),
            (0, "http://shared".to_owned()),
            (0, "http://[2a01:4f8::1]".to_owned()),
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
    let first = config(1, &["http://45.90.100.1", "http://45.90.100.2"]);
    store.save(&first).unwrap();
    // The service rotates key and addresses; the manifest announces them.
    let rotated = config(2, &["http://45.90.113.5"]);
    let mut fake = Fake::new(&[
        ("http://45.90.100.1", Endpoint::Down),
        ("http://45.90.100.2", Endpoint::Up(1)),
        ("http://45.90.113.5", Endpoint::Up(2)),
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
            ("http://45.90.100.1".into(), "".into()),
            ("http://45.90.100.2".into(), "user".into()),
            ("http://45.90.100.1".into(), "".into()),
            ("http://45.90.100.2".into(), "manifest".into()),
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
        Some(&("http://45.90.113.5".into(), "manifest".into()))
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
    let first = config(1, &["http://45.90.100.1"]);
    store.save(&first).unwrap();
    let fake = Arc::new(Fake::new(&[("http://45.90.100.1", Endpoint::Up(1))]));
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
        .save(&config(1, &["http://45.90.100.1", "http://45.90.100.2"]))
        .unwrap();
    let fake = Arc::new(Fake::new(&[]));
    assert!(client_with(&fake, &store)
        .manifest("tester", "secret")
        .is_err());
    let tried: Vec<_> = fake.calls().into_iter().map(|(u, _)| u).collect();
    assert_eq!(&tried[..2], ["http://45.90.100.1", "http://45.90.100.2"]);
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
        .save(&config(1, &["http://45.90.100.1", "http://45.90.100.2"]))
        .unwrap();
    let fake = Arc::new(Fake::new(&[
        ("http://45.90.100.1", Endpoint::Refuses(1)),
        ("http://45.90.100.2", Endpoint::Up(1)),
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
    store.save(&config(1, &["http://45.90.100.1"])).unwrap();
    let fake = Arc::new(Fake::new(&[("http://45.90.100.1", Endpoint::Up(1))]));
    client_with(&fake, &store)
        .manifest("tester", "secret")
        .unwrap();
    assert_eq!(
        fake.calls(),
        vec![("http://45.90.100.1".into(), "manifest".into())]
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
    let good = config(1, &["http://45.90.100.1"]);
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
        let changed = config(1, &["http://45.90.100.7"]);
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

#[test]
fn staff_cancellation_in_failover_pause_prevents_the_next_post() {
    let fake = Arc::new(Fake::new(&[]));
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let error = Client::with_transport(fake.clone())
        .cancellable(cancel)
        .pausing(move |_| flag.store(true, Ordering::SeqCst))
        .manifest("tester", "synthetic")
        .err()
        .unwrap();
    assert!(error.is::<Cancelled>());
    assert_eq!(fake.calls().len(), 1);
}

#[test]
fn staff_cancelled_response_must_not_replace_persisted_bootstrap() {
    let (_dir, store) = store();
    let previous = config(1, &["http://45.90.100.1"]);
    store.save(&previous).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let mut fake = Fake::new(&[("http://45.90.100.1", Endpoint::Up(1))]);
    fake.next_bootstrap = Some(config(2, &["http://45.90.113.77:8080"]));
    fake.on_call = Box::new(move |_| flag.store(true, Ordering::SeqCst));
    let result = Client::with_transport(Arc::new(fake))
        .remembering(store.clone())
        .cancellable(cancel)
        .manifest("tester", "synthetic");
    // Check disk first: epoch filtering in the GUI cannot undo this write.
    assert_eq!(store.load(), Some(previous));
    assert!(result.err().unwrap().is::<Cancelled>());
}

#[test]
fn staff_malformed_bootstrap_fields_do_not_destroy_a_usable_server_list() {
    let good = config(1, &["http://45.90.100.1"]);
    for invalid in [
        Bootstrap {
            urls: vec![],
            ..good.clone()
        },
        Bootstrap {
            urls: vec!["https://user:secret@45.90.113.1".into()],
            ..good.clone()
        },
        Bootstrap {
            exponent: "!!!".into(),
            ..good.clone()
        },
        Bootstrap {
            urls: vec!["http://45.90.113.1".into(); MAX_URLS + 1],
            ..good.clone()
        },
    ] {
        let manifest = wave_protocol::parse_manifest(&manifest_document(Some(&invalid))).unwrap();
        assert!(manifest.bootstrap.is_none());
        assert_eq!(manifest.servers[0].name, "Alpha");
    }
}

// ---- Fix pass: fenced persistence (R4) and Network Lock bootstrap inputs (R3).

#[test]
fn fix_cancel_requests_is_a_fence_for_in_flight_writes() {
    let (_dir, store) = store();
    let previous = config(1, &["http://45.90.100.1"]);
    store.save(&previous).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let mut fake = Fake::new(&[("http://45.90.100.1", Endpoint::Up(1))]);
    fake.next_bootstrap = Some(config(2, &["http://45.90.113.77:8080"]));
    // Sign-out happens while the POST is in flight, through the GUI's API.
    fake.on_call = Box::new(move |_| wave_protocol::cancel_requests(&flag));
    let result = Client::with_transport(Arc::new(fake))
        .remembering(store.clone())
        .cancellable(cancel)
        .login("tester", "synthetic");
    assert!(result.err().unwrap().is::<Cancelled>());
    assert_eq!(store.load(), Some(previous));
}

#[test]
fn fix_a_write_refused_after_cancellation_leaves_the_store_untouched() {
    let (_dir, store) = store();
    let previous = config(1, &["http://45.90.100.1"]);
    store.save(&previous).unwrap();
    let next = config(2, &["http://45.90.113.77:8080"]);
    let error = store
        .save_fenced(&next, 1, &Default::default(), store.revision(), &|| false)
        .unwrap_err();
    assert!(error.is::<Cancelled>());
    assert_eq!(store.load(), Some(previous));
}

#[test]
fn fix_an_old_session_response_cannot_roll_back_a_newer_bootstrap() {
    let (_dir, store) = store();
    let newer = config(2, &["http://45.90.113.20"]);
    // A later session already stored the bootstrap of a manifest from t=2000.
    assert_eq!(
        store
            .save_fenced(
                &newer,
                2_000,
                &Default::default(),
                store.revision(),
                &|| true
            )
            .unwrap(),
        Saved::Written
    );
    // The earlier session's response (manifest t=1000) completes afterwards.
    let mut fake = Fake::new(&[("http://45.90.113.20", Endpoint::Up(2))]);
    fake.next_bootstrap = Some(config(3, &["http://45.90.113.10"]));
    fake.manifest_time = Some(1_000);
    let manifest = client_with(&Arc::new(fake), &store)
        .manifest("tester", "synthetic")
        .unwrap();
    assert_eq!(manifest.issued, 1_000);
    assert_eq!(store.load(), Some(newer.clone()));
    assert_eq!(store.latest().issued, 2_000);
    // The same or a newer manifest is accepted.
    let mut fake = Fake::new(&[("http://45.90.113.20", Endpoint::Up(2))]);
    let rotated = config(4, &["http://45.90.113.30"]);
    fake.next_bootstrap = Some(rotated.clone());
    fake.manifest_time = Some(3_000);
    client_with(&Arc::new(fake), &store)
        .manifest("tester", "synthetic")
        .unwrap();
    assert_eq!(store.load(), Some(rotated));
}

#[test]
fn fix_concurrent_writers_keep_the_newest_manifest() {
    let (_dir, store) = store();
    let configs: Vec<_> = (0..8u64)
        .map(|i| config(2, &[&format!("http://45.90.113.{}", 40 + i)]))
        .collect();
    std::thread::scope(|scope| {
        for (i, c) in configs.iter().enumerate() {
            let store = store.clone();
            scope.spawn(move || {
                store
                    .save_fenced(c, 100 + i as u64, &Default::default(), 0, &|| true)
                    .unwrap();
            });
        }
    });
    assert_eq!(store.latest().issued, 107);
    assert_eq!(store.load(), Some(configs[7].clone()));
}

#[test]
fn fix_version_one_store_files_still_load() {
    let (_dir, store) = store();
    let latest = config(1, &["http://45.90.100.1"]);
    std::fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    std::fs::write(
        store.path(),
        serde_json::to_vec(&serde_json::json!({"version":1,"bootstrap":latest})).unwrap(),
    )
    .unwrap();
    assert_eq!(store.load(), Some(latest));
    assert_eq!(store.latest().issued, 0);
}

#[test]
fn fix_hostname_bootstrap_is_resolved_once_pinned_and_exempted() {
    let (_dir, store) = store();
    let rotated = config(
        2,
        &[
            "https://bootstrap.example:4443",
            "http://[2a01:4f8::77]:8443",
        ],
    );
    let mut fake = Fake::new(&[("http://45.90.100.1", Endpoint::Up(1))]);
    fake.next_bootstrap = Some(rotated.clone());
    store.save(&config(1, &["http://45.90.100.1"])).unwrap();
    let lookups = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen = lookups.clone();
    Client::with_transport(Arc::new(fake))
        .remembering(store.clone())
        .resolving(move |name| {
            seen.lock().unwrap().push(name.to_owned());
            Some(vec![
                "45.90.113.80".parse().unwrap(),
                "10.0.0.1".parse().unwrap(), // never exempted
                "127.0.0.1".parse().unwrap(),
                "2a01:4f8::80".parse().unwrap(),
            ])
        })
        .pausing(|_| ())
        .manifest("tester", "synthetic")
        .unwrap();
    // Only the hostname is looked up; literals need no DNS.
    assert_eq!(
        *lookups.lock().unwrap(),
        vec!["bootstrap.example".to_owned()]
    );
    let latest = store.latest();
    assert_eq!(latest.bootstrap, Some(rotated));
    assert_eq!(
        latest.resolved["bootstrap.example"],
        vec![
            "45.90.113.80".parse::<std::net::IpAddr>().unwrap(),
            "2a01:4f8::80".parse().unwrap()
        ]
    );
    let exemptions = store.exemptions();
    // Literal addresses first, then hostname addresses (shared selection).
    let expected: Vec<Exemption> = [
        ("2a01:4f8::77", 8443),
        ("45.90.113.80", 4443),
        ("2a01:4f8::80", 4443),
    ]
    .iter()
    .map(|(ip, port)| Exemption {
        ip: ip.parse().unwrap(),
        port: *port,
    })
    .collect();
    assert_eq!(exemptions, expected);
    let rules = wave_protocol::firewall::policy(
        &wave_protocol::options::Options::default(),
        None,
        true,
        &exemptions,
        Some(1000),
    )
    .unwrap();
    assert!(rules.contains("ip daddr 45.90.113.80 tcp dport 4443 accept"));
    assert!(rules.contains("ip6 daddr 2a01:4f8::80 tcp dport 4443 accept"));
    assert!(rules.contains("ip6 daddr 2a01:4f8::77 tcp dport 8443 accept"));
    assert!(!rules.contains("10.0.0.1") && !rules.contains("127.0.0.1"));
    assert!(!rules.contains("bootstrap.example"));

    // A later client pins the hostname to those addresses: no DNS is needed
    // outside the tunnel under Network Lock.
    let fake = Arc::new(Fake::new(&[]));
    let _ = Client::with_transport(fake.clone())
        .remembering(store.clone())
        .resolving(|_| panic!("no lookup without a new manifest"))
        .pausing(|_| ())
        .manifest("tester", "synthetic");
    let pinned = fake.pinned.lock().unwrap().clone();
    assert_eq!(
        pinned,
        vec![(
            "https://bootstrap.example:4443".to_owned(),
            vec![
                "45.90.113.80:4443".parse().unwrap(),
                "[2a01:4f8::80]:4443".parse().unwrap()
            ]
        )]
    );
}

#[test]
fn fix_failed_hostname_lookup_keeps_previous_addresses_only_for_listed_hosts() {
    let host = config(1, &["https://bootstrap.example"]);
    let previous = Resolved::from([
        (
            "bootstrap.example".to_owned(),
            vec!["45.90.113.5".parse().unwrap()],
        ),
        (
            "retired.example".to_owned(),
            vec!["45.90.113.6".parse().unwrap()],
        ),
    ]);
    let kept = resolve_hosts(&host, &previous, &|_| None);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept["bootstrap.example"], previous["bootstrap.example"]);
    let many: Vec<std::net::IpAddr> = (1..=9)
        .map(|i| format!("45.90.113.{i}").parse().unwrap())
        .collect();
    let bounded = resolve_hosts(&host, &Resolved::new(), &|_| Some(many.clone()));
    assert_eq!(bounded["bootstrap.example"].len(), MAX_ADDRESSES_PER_HOST);
}

#[test]
fn fix_exemption_validation_rejects_unsafe_addresses_and_ports() {
    let ok = |ip: &str, port| Exemption {
        ip: ip.parse().unwrap(),
        port,
    };
    for (ip, port) in [
        ("45.90.113.77", 8080),
        ("2a01:4f8::77", 443),
        ("63.33.78.166", 80),
    ] {
        ok(ip, port).validate().unwrap();
    }
    for (ip, port) in [
        ("0.0.0.0", 80),
        ("127.0.0.1", 80),
        ("10.1.2.3", 80),
        ("172.16.0.1", 80),
        ("192.168.1.1", 80),
        ("169.254.1.1", 80),
        ("100.64.0.1", 80),
        ("224.0.0.1", 80),
        ("255.255.255.255", 80),
        ("240.0.0.1", 80),
        ("::", 80),
        ("::1", 80),
        ("fd00::1", 80),
        ("fe80::1", 80),
        ("ff02::1", 80),
        ("::ffff:45.90.113.1", 80),
        ("45.90.113.77", 0),
        ("45.90.113.77", 53),
        ("45.90.113.77", 853),
    ] {
        let e = ok(ip, port);
        assert!(e.validate().is_err(), "accepted {ip}:{port}");
        assert!(
            wave_protocol::firewall::policy(
                &wave_protocol::options::Options::default(),
                None,
                true,
                &[e],
                Some(1000),
            )
            .is_err(),
            "policy accepted {ip}:{port}"
        );
    }
    let too_many: Vec<_> = (1..=MAX_EXEMPTIONS as u16 + 1)
        .map(|p| ok("45.90.113.77", 1000 + p))
        .collect();
    assert!(validate_exemptions(&too_many).is_err());
    assert!(validate_exemptions(&too_many[..MAX_EXEMPTIONS]).is_ok());
}

#[test]
fn fix_rotation_replaces_old_latest_exemptions_and_keeps_embedded_fallback() {
    let (_dir, store) = store();
    let options = wave_protocol::options::Options::default();
    store
        .save_fenced(
            &config(1, &["http://45.90.113.10"]),
            100,
            &Default::default(),
            store.revision(),
            &|| true,
        )
        .unwrap();
    store
        .save_fenced(
            &config(2, &["http://45.90.113.11:8081"]),
            200,
            &Default::default(),
            store.revision(),
            &|| true,
        )
        .unwrap();
    let rules =
        wave_protocol::firewall::policy(&options, None, true, &store.exemptions(), Some(1000))
            .unwrap();
    assert!(rules.contains("ip daddr 45.90.113.11 tcp dport 8081 accept"));
    assert!(
        !rules.contains("45.90.113.10"),
        "retired endpoint stays blocked"
    );
    for e in wave_protocol::firewall::embedded_exemptions() {
        assert!(rules.contains(&format!("tcp dport {} accept", e.port)));
        assert!(rules.contains(&e.ip.to_string()));
    }
    // No DNS or HTTP is opened in general: every bootstrap accept names an address.
    for line in rules
        .lines()
        .filter(|l| l.contains("tcp dport") && l.contains("accept"))
    {
        assert!(line.contains(" daddr "), "{line}");
        assert!(!line.contains("dport 53") && !line.contains("dport 853"));
    }
}

// F2 wiring: under Network Lock outside the tunnel a hostname without a pin
// is skipped (never ordinary DNS); a pinned hostname uses only its pins.
#[test]
fn recheck_required_pins_skip_unpinned_hostnames() {
    let (_dir, store) = store();
    store
        .save(&config(
            1,
            &["https://pinned.example:4443", "http://45.90.100.1"],
        ))
        .unwrap();
    // Without a stored pin the hostname is never contacted when pins are required.
    let fake = Arc::new(Fake::new(&[("http://45.90.100.1", Endpoint::Up(1))]));
    client_with(&fake, &store)
        .requiring_pins(true)
        .resolving(|_| None)
        .manifest("tester", "synthetic")
        .unwrap();
    assert!(fake.calls().iter().all(|(u, _)| !u.contains("example")));
    assert!(fake.pinned.lock().unwrap().is_empty());
    // The embedded definition's hostname (bootme.org) is skipped too.
    let fake = Arc::new(Fake::new(&[]));
    let _ = client_with(&fake, &store)
        .requiring_pins(true)
        .manifest("tester", "synthetic");
    assert!(fake
        .calls()
        .iter()
        .all(|(u, _)| !u.contains("bootme.org") && !u.contains("example")));
    // Not required (no lock or inside the tunnel): ordinary resolution is allowed.
    let fake = Arc::new(Fake::new(&[]));
    let _ = client_with(&fake, &store).manifest("tester", "synthetic");
    assert!(fake
        .calls()
        .iter()
        .any(|(u, _)| u.contains("pinned.example")));
}

// F4 wiring: the base revision is captured before the request, so a write by
// another session during the request wins an equal manifest time.
#[test]
fn recheck_write_during_request_fences_an_equal_time_response() {
    let (_dir, store) = store();
    store.save(&config(1, &["http://45.90.100.1"])).unwrap();
    let concurrent = config(2, &["http://45.90.100.2"]);
    let mut fake = Fake::new(&[("http://45.90.100.1", Endpoint::Up(1))]);
    fake.next_bootstrap = Some(config(3, &["http://45.90.100.3"]));
    fake.manifest_time = Some(500);
    let writer = store.clone();
    let other = concurrent.clone();
    fake.on_call = Box::new(move |_| {
        let base = writer.revision();
        writer
            .save_fenced(&other, 500, &Default::default(), base, &|| true)
            .unwrap();
    });
    client_with(&Arc::new(fake), &store)
        .manifest("tester", "synthetic")
        .unwrap();
    assert_eq!(store.load(), Some(concurrent));
}

// Bounded adoption: once the budget predicate turns false no new lookup
// starts; remaining hosts keep previous addresses.
#[test]
fn recheck_hostname_resolution_stops_at_the_budget() {
    let b = config(
        1,
        &[
            "https://a.example",
            "https://b.example",
            "https://c.example",
        ],
    );
    let previous: Resolved = [("c.example".to_owned(), vec!["8.8.8.3".parse().unwrap()])].into();
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let resolved = wave_protocol::bootstrap::resolve_hosts_bounded(
        &b,
        &previous,
        &|_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Some(vec!["8.8.8.1".parse().unwrap()])
        },
        &|| calls.load(Ordering::SeqCst) < 1,
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(resolved.len(), 2, "a.example looked up, c.example kept");
    assert_eq!(resolved["c.example"], previous["c.example"]);
    assert!(wave_protocol::bootstrap::RESOLVE_BUDGET <= Duration::from_secs(6));
}
