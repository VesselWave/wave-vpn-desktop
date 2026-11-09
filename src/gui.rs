// SPDX-License-Identifier: GPL-3.0-or-later
// Fresh controller and views. The archived app is used only as a behavior/visual reference.
mod icons;
mod preferences;
mod preview;
mod theme;
mod tray;
#[cfg(target_os = "linux")]
mod tray_icon;
mod views;
use iced::{Element, Subscription, Task, Theme};
use preferences::{Preferences, Target};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, Write},
    process::{ChildStdin, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};
use wave_protocol::{
    bootstrap::Exemption,
    latency::{self, LatencyCache, LatencyStore},
    options::{Options, Section},
    schedule::{self, RefreshSchedule},
    Account, BootstrapStore, Client, Manifest, Server, TunnelProfile,
};
use zeroize::Zeroize;
#[derive(Clone, Copy, Debug, PartialEq)]
enum Tab {
    Overview,
    Servers,
    Logs,
    Settings,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum ServerView {
    All,
    Favorites,
    Countries,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Sort {
    Name,
    Country,
    Load,
    Users,
    Ping,
    Score,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Level {
    Debug,
    Info,
    Warn,
    Error,
}
impl Level {
    fn label(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}
#[derive(Clone)]
struct Log {
    time: String,
    level: Level,
    text: String,
}
#[derive(Clone)]
enum Message {
    Tick,
    Login(String),
    Password(String),
    SignIn,
    ImportPath(String),
    ImportPassword(String),
    ImportEddie,
    SignOut,
    Page(Tab),
    Search(String),
    Select(Target),
    Device(String),
    Favorite(String),
    Connect,
    Disconnect,
    Recover,
    Refresh,
    ServerView(ServerView),
    Sort(Sort),
    Filter(Level),
    Clear,
    Diagnostics,
    DiagnosticsClose,
    DiagnosticsPath(String),
    DiagnosticsCopy,
    DiagnosticsSave,
    Section(Section),
    Option(&'static str, String),
    Save,
    LockAction(&'static str),
    Resize(iced::Size),
    SystemTheme(iced::theme::Mode),
    Capture(iced::window::Screenshot),
    Close(iced::window::Id),
    Show,
    Quit,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Wave UI event")
    }
}
enum Event {
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Tray(tray::Action),
    TrayReady(Option<tray::Handle>),
    Login(u64, Box<Result<(Account, Manifest), String>>),
    /// Periodic refreshes fetch only the manifest; a manual one also the account.
    Refreshed(u64, bool, Box<Result<(Option<Account>, Manifest), String>>),
    Imported(u64, Result<wave_protocol::eddie::Imported, String>),
    AccountSaved(u64, Result<(), String>),
    StorageLoaded(
        u64,
        wave_protocol::credentials::Storage,
        Result<Option<wave_protocol::credentials::Credentials>, String>,
    ),
    SignedOut(u64, Result<(), String>),
    Pings(u64, latency::Outcome),
    Line(String),
    End(String),
    /// A privileged action finished. Carries the exemption generation it
    /// sent, if any, so only that exact set is confirmed on success.
    Notice(Option<u64>, Result<String, String>),
    NetworkStatus(u64, Result<wave_protocol::protection::Status, String>),
    DiagnosticsSaved(Result<(), String>),
}
#[derive(Default)]
struct WorkerControl {
    cancelled: bool,
    release: bool,
    input: Option<ChildStdin>,
}
impl WorkerControl {
    fn stop(&mut self) {
        self.command(true);
    }
    fn command(&mut self, release: bool) {
        self.cancelled = true;
        self.release = release;
        if let Some(mut input) = self.input.take() {
            let _ = input.write_all(if release {
                b"disconnect\n"
            } else {
                b"switch\n"
            });
        }
    }
}
struct Wave {
    account_epoch: u64,
    quitting: bool,
    switching: bool,
    retry_at: Option<Instant>,
    penalties: BTreeMap<String, Instant>,
    session_server: Option<String>,
    guarded: bool,
    protection_known: bool,
    protection: wave_protocol::protection::Status,
    protection_revision: Option<u64>,
    status_pending: bool,
    network_revision: u64,
    status_checked: Instant,
    checks: wave_protocol::checks::Results,
    retryable: bool,
    /// A helper refusal seen during this connection, explained on its end.
    helper_problem: Option<&'static str>,
    retry_attempts: u32,
    diagnostics: Option<String>,
    diagnostics_path: String,
    last_failure: wave_protocol::diagnostics::Failure,
    window: Option<iced::window::Id>,
    tray: Option<tray::Handle>,
    login: String,
    password: String,
    import_path: String,
    import_password: String,
    importing: bool,
    keyring_busy: bool,
    storage: Option<wave_protocol::credentials::Storage>,
    network_busy: bool,
    lock_change: Option<bool>,
    session_password: String,
    busy: bool,
    refresh: RefreshSchedule,
    /// When the account document (devices, subscription) was last fetched.
    account_checked: Instant,
    /// Set on sign-out so in-flight sign-in/refresh requests stop failing over.
    requests_cancel: Arc<AtomicBool>,
    /// Latest manifest bootstrap; tests point it at a temporary directory.
    bootstrap: BootstrapStore,
    /// Exemptions the helper confirmed applying (None: not yet this launch).
    applied_exemptions: Option<Vec<Exemption>>,
    /// Sent to the helper and awaiting its result, with its generation.
    exemptions_sent: Option<(u64, Vec<Exemption>)>,
    exemptions_generation: u64,
    /// Handed to a connection worker; confirmed by its protection event.
    connection_exemptions: Option<Vec<Exemption>>,
    /// Consecutive failed updates and the earliest idle retry.
    exemptions_failures: u32,
    exemptions_retry_at: Option<Instant>,
    /// The confirmed set differs from the latest; update when idle.
    exemptions_pending: bool,
    account: Option<Account>,
    manifest: Option<Manifest>,
    tab: Tab,
    section: Section,
    server_view: ServerView,
    sort: Sort,
    ascending: bool,
    search: String,
    prefs: Preferences,
    draft: Options,
    latency: LatencyCache,
    latency_cancel: Arc<AtomicBool>,
    /// Persisted results and cooldown; None in preview and unit tests.
    latency_store: Option<LatencyStore>,
    logs: Vec<Log>,
    filters: BTreeSet<u8>,
    status: String,
    error: Option<String>,
    activity: String,
    events: mpsc::Receiver<Event>,
    sender: mpsc::Sender<Event>,
    control: Option<Arc<Mutex<WorkerControl>>>,
    width: f32,
    system_dark: bool,
    rx: u64,
    tx: u64,
    rx_rate: f64,
    tx_rate: f64,
    last_stats: Instant,
    since: Option<Instant>,
    connections: u32,
    active_server: Option<Server>,
    capture_tick: usize,
    preview: bool,
    persistent_lock: bool,
    lock_installed: bool,
    /// Tests: pretend the helper is (not) installed and record privileged
    /// actions with their input instead of running them.
    #[cfg(test)]
    test_helper: Option<bool>,
    #[cfg(test)]
    test_privileged: Vec<(&'static str, wave_protocol::transport::LockInput)>,
}
impl Wave {
    fn new() -> Self {
        let (sender, events) = mpsc::channel();
        let prefs = Preferences::load();
        let draft = prefs.options.clone();
        let mut app = Self {
            account_epoch: 0,
            quitting: false,
            switching: false,
            retry_at: None,
            penalties: BTreeMap::new(),
            session_server: None,
            guarded: false,
            protection_known: false,
            protection: Default::default(),
            protection_revision: None,
            status_pending: false,
            network_revision: 0,
            status_checked: Instant::now() - Duration::from_secs(10),
            checks: Default::default(),
            retryable: false,
            helper_problem: None,
            retry_attempts: 0,
            diagnostics: None,
            diagnostics_path: String::new(),
            last_failure: Default::default(),
            window: None,
            tray: None,
            login: String::new(),
            password: String::new(),
            import_path: String::new(),
            import_password: String::new(),
            importing: false,
            keyring_busy: false,
            storage: None,
            network_busy: false,
            lock_change: None,
            session_password: String::new(),
            busy: false,
            refresh: RefreshSchedule::new(Instant::now()),
            account_checked: Instant::now(),
            requests_cancel: Arc::new(AtomicBool::new(false)),
            bootstrap: BootstrapStore::in_directory(&preferences::directory()),
            applied_exemptions: None,
            exemptions_sent: None,
            exemptions_generation: 0,
            connection_exemptions: None,
            exemptions_failures: 0,
            exemptions_retry_at: None,
            exemptions_pending: false,
            account: None,
            manifest: None,
            tab: Tab::Overview,
            section: Section::General,
            server_view: ServerView::All,
            sort: Sort::Score,
            ascending: true,
            search: String::new(),
            prefs,
            draft,
            latency: LatencyCache::default(),
            latency_cancel: Arc::new(AtomicBool::new(false)),
            latency_store: None,
            logs: vec![],
            filters: BTreeSet::from([1, 2, 3]),
            status: "Disconnected".into(),
            error: None,
            activity: String::new(),
            events,
            sender,
            control: None,
            width: 900.,
            system_dark: true,
            rx: 0,
            tx: 0,
            rx_rate: 0.,
            tx_rate: 0.,
            last_stats: Instant::now(),
            since: None,
            connections: 0,
            active_server: None,
            capture_tick: 0,
            preview: std::env::var_os("WAVE_PREVIEW").is_some(),
            persistent_lock: false,
            lock_installed: false,
            #[cfg(test)]
            test_helper: None,
            #[cfg(test)]
            test_privileged: vec![],
        };
        if app.preview {
            preview::setup(&mut app);
        }
        app
    }
    fn boot() -> (Self, Task<Message>) {
        let mut app = Self::new();
        let (id, open) = iced::window::open(window_settings());
        app.window = Some(id);
        if !app.preview {
            // A restart reuses recent results and honours the probe cooldown.
            let store = LatencyStore::in_directory(&preferences::directory());
            store.load(&mut app.latency, Instant::now());
            app.latency_store = Some(store);
            app.keyring_busy = true;
            let tx = app.sender.clone();
            let epoch = app.account_epoch;
            std::thread::spawn(move || {
                let storage = wave_protocol::credentials::Storage::detect();
                let loaded = storage.load(&preferences::directory()).map_err(|_| {
                    "Unable to restore your saved account. Unlock your keyring and try again."
                        .to_owned()
                });
                let _ = tx.send(Event::StorageLoaded(epoch, storage, loaded));
            });
        }
        let tx = app.sender.clone();
        std::thread::spawn(move || {
            let handle = tray::spawn(tx.clone());
            let _ = tx.send(Event::TrayReady(handle));
        });
        (
            app,
            Task::batch([
                open.discard(),
                iced::system::theme().map(Message::SystemTheme),
            ]),
        )
    }
    fn log(&mut self, level: Level, line: String) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            % 86400;
        let record = Log {
            time: format!("{:02}:{:02}:{:02}", now / 3600, (now / 60) % 60, now % 60),
            level,
            text: line,
        };
        if !self.preview
            && self.prefs.options.on("log_file")
            && (level != Level::Debug || self.prefs.options.on("debug"))
        {
            let path = if self.prefs.options.get("log_path").is_empty() {
                preferences::directory().join("wave.log")
            } else {
                self.prefs.options.get("log_path").into()
            };
            if let Err(e) = preferences::append_log(
                &path,
                &format!("{} {} {}", record.time, level.label(), record.text),
            ) {
                self.error = Some(format!("File logging unavailable: {e}"));
            }
        }
        self.activity = record.text.clone();
        self.logs.push(record);
        if self.logs.len() > 10000 {
            self.logs.remove(0);
        }
    }
    fn persist(&mut self) {
        if !self.preview {
            if let Err(e) = self.prefs.save() {
                self.error = Some(e.to_string());
            }
        }
    }
    fn dirty(&self) -> bool {
        self.draft != self.prefs.options
    }
    fn server_score(&self, server: &Server) -> Option<u32> {
        self.manifest.as_ref()?.desktop_score(
            server,
            self.latency.ranking(&server.name),
            self.prefs.options.number("capacity_factor"),
        )
    }
    fn candidates(&self) -> Vec<&Server> {
        let Some(m) = &self.manifest else {
            return vec![];
        };
        let codes = |key: &str| {
            self.prefs
                .options
                .get(key)
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        };
        let allow = codes("countries_allow");
        let deny = codes("countries_deny");
        let mut servers: Vec<_> = m
            .servers
            .iter()
            .filter(|s| {
                s.available()
                    && (allow.is_empty() || allow.contains(&s.country))
                    && !deny.contains(&s.country)
            })
            .collect();
        servers.sort_by(|a, b| {
            self.penalties
                .get(&a.name)
                .is_some_and(|t| *t > Instant::now())
                .cmp(
                    &self
                        .penalties
                        .get(&b.name)
                        .is_some_and(|t| *t > Instant::now()),
                )
                .then(
                    self.server_score(a)
                        .unwrap_or(u32::MAX)
                        .cmp(&self.server_score(b).unwrap_or(u32::MAX)),
                )
                .then(a.name.cmp(&b.name))
        });
        servers
    }
    fn selected(&self) -> Option<&Server> {
        let candidates = self.candidates();
        if self.prefs.options.on("locklast") {
            if let Some(name) = &self.session_server {
                return candidates.into_iter().find(|s| &s.name == name);
            }
        }
        match &self.prefs.target {
            Target::Best => candidates.first().copied(),
            Target::Country(c) => candidates.into_iter().find(|s| &s.country == c),
            Target::Server(n) => candidates.into_iter().find(|s| &s.name == n),
        }
    }
    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tick => {
                while let Ok(e) = self.events.try_recv() {
                    match e {
                        Event::DiagnosticsSaved(result) => match result {
                            Ok(()) => {
                                self.error = None;
                                self.log(Level::Info, "Diagnostic report saved".into());
                            }
                            Err(error) => {
                                self.last_failure = wave_protocol::diagnostics::Failure::Export;
                                self.error = Some(error);
                            }
                        },
                        Event::NetworkStatus(revision, result) => {
                            self.status_pending = false;
                            if revision != self.network_revision {
                                continue;
                            }
                            self.status_checked = Instant::now();
                            match result {
                                Ok(status) => {
                                    self.protection_known = status.enabled.is_some();
                                    if let Some(enabled) = status.enabled {
                                        self.guarded = enabled;
                                    }
                                    self.persistent_lock = status.persistent;
                                    self.protection = status;
                                    self.protection_revision = Some(revision);
                                    // First confirmed lock this launch (e.g. restored at
                                    // boot): make sure it allows the latest bootstrap.
                                    if (self.guarded || self.persistent_lock)
                                        && self.applied_exemptions.is_none()
                                        && self.account.is_some()
                                    {
                                        self.exemptions_pending = true;
                                    }
                                }
                                Err(_) => {
                                    self.protection_known = false;
                                    self.protection.enabled = None;
                                    self.protection_revision = None;
                                }
                            }
                        }
                        Event::TrayReady(handle) => self.tray = handle,
                        Event::Tray(action) => {
                            let message = match action {
                                tray::Action::Show => Message::Show,
                                tray::Action::Connect => Message::Connect,
                                tray::Action::Disconnect => Message::Disconnect,
                                tray::Action::Servers => {
                                    self.tab = Tab::Servers;
                                    Message::Show
                                }
                                tray::Action::Settings => {
                                    self.tab = Tab::Settings;
                                    Message::Show
                                }
                                tray::Action::Recover => Message::Recover,
                                tray::Action::Quit => Message::Quit,
                            };
                            return self.update(message);
                        }
                        Event::Login(epoch, result) => {
                            if epoch != self.account_epoch {
                                continue;
                            }
                            self.busy = false;
                            match *result {
                                Ok((a, m)) => {
                                    let first = self.account.is_none();
                                    let interval = self.refresh_interval(Some(&m));
                                    self.refresh.succeeded(
                                        Instant::now(),
                                        interval,
                                        rand::random(),
                                    );
                                    self.adopt_account(a);
                                    self.latency.set_locations(&m.servers);
                                    self.manifest = Some(m);
                                    self.error = None;
                                    self.sync_lock_exemptions();
                                    self.log(
                                        Level::Info,
                                        "AirVPN account and server list updated".into(),
                                    );
                                    if first
                                        && self.prefs.options.on("startlast")
                                        && !self.prefs.last_server.is_empty()
                                    {
                                        self.prefs.target =
                                            Target::Server(self.prefs.last_server.clone());
                                    }
                                    if first {
                                        self.save_account();
                                    }
                                    if first && self.prefs.options.on("autoconnect") {
                                        return self.update(Message::Connect);
                                    }
                                }
                                Err(e) => {
                                    self.last_failure = if e.contains("rejected") {
                                        wave_protocol::diagnostics::Failure::AccountRejected
                                    } else {
                                        wave_protocol::diagnostics::Failure::SignInNetwork
                                    };
                                    self.error = Some(e.clone());
                                    self.log(Level::Error, e);
                                    if self.account.is_none() {
                                        self.session_password.zeroize();
                                    }
                                }
                            }
                        }
                        Event::Refreshed(epoch, manual, result) => {
                            if epoch != self.account_epoch {
                                continue;
                            }
                            match *result {
                                Ok((account, m)) => {
                                    let interval = self.refresh_interval(Some(&m));
                                    self.refresh.succeeded(
                                        Instant::now(),
                                        interval,
                                        rand::random(),
                                    );
                                    if let Some(a) = account {
                                        self.adopt_account(a);
                                    }
                                    self.latency.set_locations(&m.servers);
                                    self.manifest = Some(m);
                                    self.sync_lock_exemptions();
                                    if self
                                        .error
                                        .as_deref()
                                        .is_some_and(|e| e.contains(KEEPING_SERVERS))
                                    {
                                        self.error = None;
                                    }
                                    self.log(Level::Info, "AirVPN server list updated".into());
                                }
                                Err(e) => {
                                    // Keep the cached account and servers usable;
                                    // retry later with growing, jittered delays.
                                    let interval = self.refresh_interval(self.manifest.as_ref());
                                    self.refresh
                                        .failed(Instant::now(), interval, rand::random());
                                    let wait = self
                                        .refresh
                                        .next()
                                        .saturating_duration_since(Instant::now());
                                    let message = format!(
                                        "{e}. {KEEPING_SERVERS}; next attempt in about {} min.",
                                        wait.as_secs().div_ceil(60)
                                    );
                                    // An account refusal needs the user's attention
                                    // even when the refresh was automatic.
                                    if manual || e.contains("rejected") {
                                        self.error = Some(message.clone());
                                    }
                                    self.log(Level::Warn, message);
                                }
                            }
                        }
                        Event::AccountSaved(epoch, result) => {
                            if epoch != self.account_epoch {
                                continue;
                            }
                            self.keyring_busy = false;
                            if let Err(error) = result {
                                self.error = Some(error);
                            }
                        }
                        Event::StorageLoaded(epoch, storage, result) => {
                            if epoch != self.account_epoch {
                                continue;
                            }
                            self.keyring_busy = false;
                            self.storage = Some(storage);
                            match result {
                                Ok(Some(mut account)) => {
                                    self.login = std::mem::take(&mut account.login);
                                    self.password.zeroize();
                                    self.password = std::mem::take(&mut account.password);
                                    return self.update(Message::SignIn);
                                }
                                Ok(None) => (),
                                Err(error) => self.error = Some(error),
                            }
                        }
                        Event::SignedOut(epoch, result) => {
                            if epoch != self.account_epoch {
                                continue;
                            }
                            self.keyring_busy = false;
                            match result {
                                Ok(()) => {
                                    self.account_epoch = self.account_epoch.wrapping_add(1);
                                    self.busy = false;
                                    // Fenced: no request of this session can still
                                    // write bootstrap state once this returns.
                                    wave_protocol::cancel_requests(&self.requests_cancel);
                                    self.requests_cancel = Arc::new(AtomicBool::new(false));
                                    self.applied_exemptions = None;
                                    self.exemptions_sent = None;
                                    self.exemptions_generation += 1;
                                    self.connection_exemptions = None;
                                    self.exemptions_failures = 0;
                                    self.exemptions_retry_at = None;
                                    self.exemptions_pending = false;
                                    self.refresh.reset(Instant::now());
                                    self.retry_at = None;
                                    self.retry_attempts = 0;
                                    self.status = "Disconnected".into();
                                    self.checks = Default::default();
                                    self.session_server = None;
                                    self.account = None;
                                    self.manifest = None;
                                    self.login.zeroize();
                                    self.password.zeroize();
                                    self.session_password.zeroize();
                                    self.cancel_measurement();
                                    self.latency.forget();
                                    self.save_latency();
                                    self.log(Level::Info, "Signed out".into());
                                }
                                Err(error) => self.error = Some(error),
                            }
                        }
                        Event::Imported(epoch, result) => {
                            self.importing = false;
                            if epoch != self.account_epoch {
                                continue;
                            }
                            match result {
                                Ok(mut imported) => {
                                    self.login = std::mem::take(&mut imported.login);
                                    self.password.zeroize();
                                    self.password = std::mem::take(&mut imported.password);
                                    self.draft = imported.options.desktop();
                                    self.error = Some(format!("Imported into the sign-in form and unsaved settings. Advanced Eddie settings are not applied; {} other options were skipped. Review, sign in and Save explicitly. Credentials have not been stored.", imported.ignored));
                                    self.log(
                                        Level::Info,
                                        "Eddie profile decoded; awaiting user review".into(),
                                    );
                                }
                                Err(e) => self.error = Some(e),
                            }
                        }
                        Event::Pings(generation, outcome) => {
                            let offline = outcome == latency::Outcome::Offline;
                            if !self.latency.finish(generation, Instant::now(), outcome) && offline
                            {
                                self.log(
                                    Level::Debug,
                                    "No server answered the latency probe; retrying later".into(),
                                );
                            }
                            self.save_latency();
                        }
                        Event::Line(line) => self.transport_line(line),
                        Event::End(message) => {
                            self.network_busy = false;
                            self.lock_change = None;
                            self.connection_exemptions = None;
                            // Re-check once idle: only calls the helper if the
                            // confirmed set is not the latest.
                            if self.guarded || self.persistent_lock {
                                self.exemptions_pending = true;
                            }
                            if self.quitting {
                                return iced::exit();
                            }
                            let cancelled = self
                                .control
                                .as_ref()
                                .is_some_and(|c| c.lock().unwrap().cancelled);
                            let retry = !message.is_empty()
                                && self.retryable
                                && !cancelled
                                && self.helper_problem.is_none()
                                && self.prefs.options.on("reconnect");
                            // Penalize a server only for a failure or an explicit
                            // switch away, never for a user Disconnect or Cancel.
                            let failed = !message.is_empty() && !cancelled;
                            if let Some(server) = self
                                .active_server
                                .as_ref()
                                .filter(|_| failed || self.switching)
                            {
                                self.penalties.insert(
                                    server.name.clone(),
                                    Instant::now()
                                        + Duration::from_secs(self.prefs.options.number("penalty")),
                                );
                            }
                            self.retry_at = retry.then(|| {
                                Instant::now()
                                    + wave_protocol::lifecycle::retry_delay_jittered(
                                        self.retry_attempts,
                                        rand::random(),
                                    )
                            });
                            if retry {
                                self.retry_attempts = self.retry_attempts.saturating_add(1);
                            }
                            self.protection_known = false;
                            self.status_checked = Instant::now() - Duration::from_secs(10);
                            if !message.is_empty() {
                                self.last_failure = wave_protocol::diagnostics::Failure::Connection;
                            }
                            self.network_revision = self.network_revision.wrapping_add(1);
                            self.control = None;
                            if self.switching && message.is_empty() {
                                self.switching = false;
                                self.session_server = None;
                                self.since = None;
                                return self.update(Message::Connect);
                            }
                            self.switching = false;
                            self.status = if retry {
                                "Reconnecting…"
                            } else {
                                "Disconnected"
                            }
                            .into();
                            self.active_server = None;
                            self.since = None;
                            self.rx_rate = 0.;
                            self.tx_rate = 0.;
                            let helper = self.helper_problem.take();
                            if !message.is_empty() {
                                let message = helper
                                    .or_else(|| helper_problem(&message))
                                    .map(str::to_owned)
                                    .or_else(|| self.checks.failure().map(str::to_owned))
                                    .unwrap_or(message);
                                self.error = Some(message.clone());
                                self.log(Level::Error, message);
                            } else {
                                self.log(Level::Info, "Disconnected".into());
                            }
                        }
                        Event::Notice(generation, result) => {
                            self.network_busy = false;
                            let outcome = generation.map(|g| (g, result.as_ref().err().cloned()));
                            self.protection_known = false;
                            self.protection_revision = None;
                            self.status_checked = Instant::now() - Duration::from_secs(10);
                            if result.is_err() {
                                self.lock_change = None;
                            }
                            match result {
                                Ok(text) => {
                                    for line in text.lines() {
                                        self.transport_line(line.to_owned());
                                    }
                                }
                                Err(e) => {
                                    self.last_failure =
                                        wave_protocol::diagnostics::Failure::Recovery;
                                    self.error = Some(
                                        helper_problem(&e).map(str::to_owned).unwrap_or(e.clone()),
                                    );
                                    self.log(Level::Error, e);
                                }
                            }
                            if let Some((generation, error)) = outcome {
                                self.exemptions_outcome(generation, error.as_ref());
                            }
                        }
                    }
                }
                let snapshot = self.tray_snapshot();
                if let Some(tray) = &mut self.tray {
                    tray.update(snapshot);
                }
                if self.window.is_none() && !self.tray.as_ref().is_some_and(|t| t.visible()) {
                    return self.update(Message::Show);
                }
                if self.exemptions_pending
                    && self.control.is_none()
                    && !self.network_busy
                    && self.exemptions_retry_at.is_none_or(|t| Instant::now() >= t)
                {
                    self.sync_lock_exemptions();
                }
                if self.preview {
                    return preview::tick(self);
                }
                if self.status_checked.elapsed() >= Duration::from_secs(15) {
                    self.protection_known = false;
                    self.protection.enabled = None;
                    self.protection_revision = None;
                }
                if !self.status_pending && self.status_checked.elapsed() >= Duration::from_secs(5) {
                    self.query_status();
                }
                if self
                    .since
                    .is_some_and(|t| t.elapsed() >= Duration::from_secs(60))
                    && self.status == "Connected"
                {
                    self.retry_attempts = 0;
                }
                if self.retry_at.is_some_and(|t| Instant::now() >= t) && self.control.is_none() {
                    self.retry_at = None;
                    return self.update(Message::Connect);
                }
                let now = Instant::now();
                // Do not race connection setup; refresh once the tunnel is up or idle.
                let connecting = self.control.is_some() && self.since.is_none();
                if self.account.is_some() && !connecting && self.refresh.due(now) {
                    self.refresh(false);
                }
                if self.account.is_some()
                    && self.manifest.is_some()
                    && self.latency.due(now, self.probes_blocked())
                {
                    self.measure();
                }
            }
            Message::ImportPath(value) => self.import_path = value,
            Message::ImportPassword(value) => {
                self.import_password.zeroize();
                self.import_password = value;
            }
            Message::ImportEddie if !self.importing && self.account.is_none() && !self.preview => {
                self.importing = true;
                let path = self.import_path.clone();
                let password = zeroize::Zeroizing::new(std::mem::take(&mut self.import_password));
                let epoch = self.account_epoch;
                let tx = self.sender.clone();
                std::thread::spawn(move || {
                    let result = wave_protocol::eddie::read(std::path::Path::new(&path), &password)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(Event::Imported(epoch, result));
                });
            }
            Message::Login(value) => self.login = value,
            Message::Password(value) => {
                self.password.zeroize();
                self.password = value;
            }
            Message::SignIn if !self.busy && !self.keyring_busy && !self.preview => {
                if self.login.trim().is_empty() || self.password.is_empty() {
                    self.error = Some("Enter your username and password".into());
                    return Task::none();
                }
                self.account_epoch = self.account_epoch.wrapping_add(1);
                let epoch = self.account_epoch;
                self.busy = true;
                self.error = None;
                self.session_password.zeroize();
                self.session_password = std::mem::take(&mut self.password);
                let login = self.login.trim().to_owned();
                self.login = login.clone();
                let mut password = self.session_password.clone();
                let tx = self.sender.clone();
                let timeout = self.prefs.options.number("http_timeout");
                let cancel = self.requests_cancel.clone();
                let store = self.bootstrap.clone();
                let pins = self.pins_required();
                std::thread::spawn(move || {
                    let r = Client::with_timeout(timeout)
                        .map(|c| {
                            c.remembering(store)
                                .cancellable(cancel)
                                .requiring_pins(pins)
                        })
                        .and_then(|c| c.login(&login, &password))
                        .map_err(|error| wave_protocol::sign_in_error(&error));
                    password.zeroize();
                    let _ = tx.send(Event::Login(epoch, Box::new(r)));
                });
            }
            Message::SignOut if self.control.is_none() && !self.keyring_busy && !self.preview => {
                if let Some(storage) = self.storage {
                    self.keyring_busy = true;
                    let epoch = self.account_epoch;
                    let tx = self.sender.clone();
                    std::thread::spawn(move || {
                        let result = storage.forget(&preferences::directory()).map_err(|_| "Could not remove your saved account. Unlock your keyring and sign out again.".to_owned());
                        let _ = tx.send(Event::SignedOut(epoch, result));
                    });
                }
            }
            Message::Page(tab) => self.tab = tab,
            Message::Section(section) => {
                self.tab = Tab::Settings;
                self.section = section;
            }
            Message::Search(value) => self.search = value,
            Message::ServerView(value) => self.server_view = value,
            Message::Sort(value) => {
                if value == self.sort {
                    self.ascending = !self.ascending;
                } else {
                    self.sort = value;
                    self.ascending = true;
                }
            }
            Message::Select(target) => {
                self.prefs.target = target;
                self.persist();
            }
            Message::Device(value) => {
                self.prefs.device = value;
                self.persist();
            }
            Message::Favorite(name) => {
                if !self.prefs.favorites.remove(&name) {
                    self.prefs.favorites.insert(name);
                }
                self.persist();
            }
            Message::Filter(level) => {
                if !self.filters.remove(&(level as u8)) {
                    self.filters.insert(level as u8);
                }
            }
            Message::Clear => self.logs.clear(),
            Message::Diagnostics => {
                let context = wave_protocol::diagnostics::Context {
                    connected: self.since.is_some() && self.status == "Connected",
                    reconnecting: self.retry_at.is_some(),
                    protection: self.protection.clone(),
                    checks: self.checks.clone(),
                    last_failure: self.last_failure,
                    bootstrap_exceptions: self.bootstrap_exceptions(),
                    bootstrap_exceptions_pending: self.exemptions_pending
                        || self.exemptions_sent.is_some(),
                };
                self.diagnostics = Some(
                    serde_json::to_string_pretty(&wave_protocol::diagnostics::report(&context))
                        .unwrap(),
                );
                self.diagnostics_path = std::env::var_os("HOME")
                    .map(std::path::PathBuf::from)
                    .filter(|path| path.is_absolute() && path.is_dir())
                    .unwrap_or_else(std::env::temp_dir)
                    .join(format!(
                        "diagnostics-{}.json",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                    ))
                    .to_string_lossy()
                    .into();
            }
            Message::DiagnosticsClose => self.diagnostics = None,
            Message::DiagnosticsPath(path) => self.diagnostics_path = path,
            Message::DiagnosticsCopy => {
                if let Some(report) = &self.diagnostics {
                    return iced::clipboard::write(report.clone());
                }
            }
            Message::DiagnosticsSave => {
                if let Some(report) = &self.diagnostics {
                    let report: serde_json::Value = serde_json::from_str(report).unwrap();
                    let path = std::path::PathBuf::from(&self.diagnostics_path);
                    let tx = self.sender.clone();
                    std::thread::spawn(move || {
                        let result = wave_protocol::diagnostics::export_report(&path, &report).map_err(|_| "Could not save report. Choose a new writable filename; existing files are never overwritten.".into());
                        let _ = tx.send(Event::DiagnosticsSaved(result));
                    });
                }
            }
            Message::Option(key, value) => self.draft.set(key, value),
            Message::Save => match self.draft.validate() {
                Ok(()) => {
                    self.prefs.options = self.draft.clone();
                    self.persist();
                    self.log(Level::Info, "Settings saved".into());
                }
                Err(e) => self.error = Some(e.to_string()),
            },
            Message::Refresh => {
                if self.refresh.manual_allowed(Instant::now()) {
                    self.refresh(true);
                } else if !self.refresh.running() {
                    self.log(
                        Level::Info,
                        "Server list was refreshed moments ago; try again in a minute".into(),
                    );
                }
            }
            Message::Connect if self.network_busy && self.control.is_none() => return Task::none(),
            Message::Connect if self.control.is_some() && !self.preview => {
                self.switching = true;
                self.retry_at = None;
                if let Some(c) = &self.control {
                    c.lock().unwrap().command(false);
                }
                self.status = "Switching server…".into();
            }
            Message::Connect if !self.preview => {
                if self.account.is_none() {
                    self.tab = Tab::Settings;
                    self.section = Section::General;
                    self.error = Some("Sign in to AirVPN to connect".into());
                    return Task::none();
                }
                match self.profile() {
                    Ok(profile) => {
                        // Probes through or around a tunnel skew the ranking.
                        self.cancel_measurement();
                        self.network_revision = self.network_revision.wrapping_add(1);
                        self.checks = Default::default();
                        self.retryable = false;
                        self.helper_problem = None;
                        let checks = self
                            .manifest
                            .as_ref()
                            .and_then(|m| self.selected().and_then(|s| m.provider_check(s)));
                        let control = Arc::new(Mutex::new(WorkerControl::default()));
                        self.control = Some(control.clone());
                        self.status = "Connecting…".into();
                        self.error = None;
                        self.active_server = self.selected().cloned();
                        if self.session_server.is_none() {
                            self.session_server =
                                self.active_server.as_ref().map(|s| s.name.clone());
                        }
                        self.retry_at = None;
                        let tx = self.sender.clone();
                        let options = self.prefs.options.clone();
                        let bootstrap = self.bootstrap.exemptions();
                        // An empty list makes the worker reuse its session
                        // set, so only a non-empty list can be confirmed.
                        self.connection_exemptions =
                            (!bootstrap.is_empty()).then(|| bootstrap.clone());
                        std::thread::spawn(move || {
                            let result = launch_transport(
                                profile,
                                options,
                                checks,
                                bootstrap,
                                control,
                                tx.clone(),
                            );
                            let _ = tx.send(Event::End(
                                result.err().map(|e| e.to_string()).unwrap_or_default(),
                            ));
                        });
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
            Message::Disconnect => {
                self.retry_attempts = 0;
                if self.control.is_none() {
                    self.status = "Disconnected".into();
                }
                self.switching = false;
                self.retry_at = None;
                self.session_server = None;
                if let Some(c) = &self.control {
                    c.lock().unwrap().stop();
                    self.status = "Disconnecting…".into();
                }
            }
            Message::Recover => {
                if self.control.is_none() && !self.network_busy {
                    self.retry_at = None;
                    self.privileged("recover");
                }
            }
            Message::LockAction(action) if !self.network_busy && !self.preview => {
                let enabled = action == "lock-enable";
                if let Some(control) = &self.control {
                    let mut control = control.lock().unwrap();
                    if let Some(input) = &mut control.input {
                        match writeln!(input, "{action}") {
                            Ok(()) => {
                                self.network_busy = true;
                                self.lock_change = Some(enabled);
                            }
                            Err(_) => {
                                self.error = Some(
                                    "Unable to change Network Lock. Connection state is updating."
                                        .into(),
                                )
                            }
                        }
                    }
                } else {
                    self.lock_change = Some(enabled);
                    self.privileged(action);
                }
            }
            Message::Resize(size) => self.width = size.width,
            Message::SystemTheme(mode) => {
                if mode != iced::theme::Mode::None {
                    self.system_dark = mode == iced::theme::Mode::Dark;
                }
            }
            Message::Capture(s) => {
                preview::save(self, s);
            }
            Message::Close(id) => {
                if self.tray.as_ref().is_some_and(|t| t.visible()) {
                    self.window = None;
                    return iced::window::close(id);
                }
                return self.update(Message::Quit);
            }
            Message::Quit => {
                if self.control.is_some() {
                    self.quitting = true;
                    return self.update(Message::Disconnect);
                }
                return iced::exit();
            }
            Message::Show => {
                if let Some(id) = self.window {
                    return iced::window::gain_focus(id);
                }
                let (id, open) = iced::window::open(window_settings());
                self.window = Some(id);
                return open.discard();
            }
            _ => (),
        }
        Task::none()
    }
    fn profile(&self) -> anyhow::Result<TunnelProfile> {
        use anyhow::Context;
        let account = self.account.as_ref().context("Sign in first")?;
        let m = self.manifest.as_ref().context("Load servers first")?;
        let s = self.selected().with_context(|| self.no_server_reason())?;
        let port = self.prefs.options.get("port");
        let mode = if port.is_empty() {
            m.modes.first()
        } else {
            m.modes.iter().find(|m| m.port.to_string() == port)
        }
        .context("Unsupported WireGuard port")?;
        let mut profile = account.profile(
            s,
            &self.prefs.device,
            mode,
            self.prefs.options.get("ipv6") == "in" && self.prefs.options.get("ipv4") == "block",
        )?;
        profile.apply_options(&self.prefs.options)?;
        Ok(profile)
    }
    /// Probing is pointless while a tunnel or Network Lock is in the way.
    fn probes_blocked(&self) -> bool {
        self.control.is_some() || self.retry_at.is_some() || self.guarded || self.network_busy
    }
    fn manual_refresh_allowed(&self) -> bool {
        self.account.is_some() && self.refresh.manual_allowed(Instant::now())
    }
    fn refresh_interval(&self, manifest: Option<&Manifest>) -> Duration {
        schedule::interval(
            self.prefs.options.number("refresh"),
            manifest.map(|m| m.refresh_minutes),
        )
    }
    fn measure(&mut self) {
        if self.preview {
            return;
        }
        let Some(targets) = self.latency_targets(rand::random()) else {
            return;
        };
        // Persist the cooldown now, under the store lock: a restart during the
        // run, or another app instance, must not probe again.
        let now = Instant::now();
        let generation = match &self.latency_store {
            Some(store) => match store.admit(&mut self.latency, now, |c| c.begin(now)) {
                Some(generation) => generation,
                None => return,
            },
            None => self.latency.begin(now),
        };
        let cancel = Arc::new(AtomicBool::new(false));
        self.latency_cancel = cancel.clone();
        let tx = self.sender.clone();
        let timeout = Duration::from_secs(self.prefs.options.number("pinger_timeout"));
        std::thread::spawn(move || {
            let prober = latency::TcpProber {
                cancel: cancel.clone(),
            };
            let outcome = latency::measure(&targets, &prober, timeout, latency::PARALLEL, &cancel);
            let _ = tx.send(Event::Pings(generation, outcome));
        });
    }
    /// A bounded sample: an explicitly selected server, then one server per
    /// location (see `LatencyCache::sample`), never the whole fleet.
    fn latency_targets(&mut self, seed: u64) -> Option<Vec<(String, std::net::SocketAddr)>> {
        let manifest = self.manifest.as_ref()?;
        self.latency.set_locations(&manifest.servers);
        let explicit = match &self.prefs.target {
            Target::Server(name) => Some(name.as_str()),
            _ => None,
        };
        Some(self.latency.sample(
            &manifest.servers,
            latency::SAMPLE_BUDGET,
            explicit.as_slice(),
            seed,
        ))
    }
    fn helper_available(&self) -> bool {
        #[cfg(test)]
        if let Some(available) = self.test_helper {
            return available;
        }
        !self.preview && wave_protocol::service::available()
    }
    fn save_latency(&mut self) {
        if let Some(store) = &self.latency_store {
            if store.save(&self.latency, Instant::now()).is_err() {
                self.log(Level::Debug, "Latency cache could not be saved".into());
            }
        }
    }
    /// Keep the selected device valid after the account document changes, so
    /// a device removed on the website does not need a sign-out to recover.
    fn adopt_account(&mut self, account: Account) {
        if !account.devices.iter().any(|d| d.name == self.prefs.device) {
            let previous = std::mem::take(&mut self.prefs.device);
            self.prefs.device = account
                .devices
                .first()
                .map(|d| d.name.clone())
                .unwrap_or_default();
            if !previous.is_empty() {
                self.log(
                    Level::Warn,
                    format!(
                        "Device {previous} is no longer in your account; using {}",
                        self.prefs.device
                    ),
                );
                self.persist();
            }
        }
        self.account = Some(account);
        self.account_checked = Instant::now();
    }
    /// Follow a bootstrap rotation under Network Lock: hand the helper the
    /// latest validated exemptions when they differ from what it was given.
    /// While a connection runs the worker owns the firewall; retry when idle.
    fn sync_lock_exemptions(&mut self) {
        self.exemptions_pending = false;
        if !(self.guarded || self.persistent_lock) {
            return;
        }
        let current = self.bootstrap.exemptions();
        if self.applied_exemptions.as_ref() == Some(&current) {
            self.exemptions_failures = 0;
            self.exemptions_retry_at = None;
            return;
        }
        // Never prompt for helper installation just to follow a rotation.
        if !self.helper_available() {
            return;
        }
        if self.control.is_some()
            || self.network_busy
            || self.exemptions_retry_at.is_some_and(|t| Instant::now() < t)
        {
            self.exemptions_pending = true;
            return;
        }
        self.privileged("lock-update");
    }
    /// Confirm a sent set only on success of the same generation. A failure
    /// keeps the last confirmed set and schedules one idle retry, backing off
    /// from 30 s to at most 15 min, never on every tick.
    fn exemptions_outcome(&mut self, generation: u64, error: Option<&String>) {
        let Some((_, sent)) = self
            .exemptions_sent
            .take_if(|(sent, _)| *sent == generation)
        else {
            return;
        };
        match error {
            None => {
                self.applied_exemptions = Some(sent);
                self.exemptions_failures = 0;
                self.exemptions_retry_at = None;
            }
            Some(error) => {
                let delay = (EXEMPTION_RETRY_MIN * 2u32.saturating_pow(self.exemptions_failures))
                    .min(EXEMPTION_RETRY_MAX);
                self.exemptions_failures = self.exemptions_failures.saturating_add(1);
                self.exemptions_retry_at = Some(Instant::now() + delay);
                self.exemptions_pending = true;
                if helper_outdated(error) {
                    self.error = Some(HELPER_OUTDATED.into());
                }
            }
        }
    }
    /// Under Network Lock without a tunnel only exempted (pinned) addresses
    /// are reachable, so hostname endpoints must not fall back to DNS.
    fn pins_required(&self) -> bool {
        (self.guarded || self.persistent_lock) && self.since.is_none()
    }
    fn idle_network_warning(&self) -> Option<&'static str> {
        // Firewall events confirm protection, but not worker/recovery state.
        // Never show a previous connection's flags while its status is refreshed.
        if self.protection_revision != Some(self.network_revision) || self.control.is_some() {
            return None;
        }
        if self.protection.worker_active {
            Some("Another Wave networking operation is active. Finish it before connecting or recovering.")
        } else if self.protection.recovery_needed {
            Some("A previous Wave connection needs cleanup. Use Recover before reconnecting; Network Lock is retained.")
        } else {
            None
        }
    }
    /// Active bootstrap exceptions to show while Network Lock is on: the
    /// compiled-in definition's and the latest set the helper confirmed.
    fn bootstrap_exceptions(&self) -> Option<usize> {
        (self.guarded || self.persistent_lock).then(|| {
            wave_protocol::firewall::embedded_exemptions()
                .into_iter()
                .chain(self.applied_exemptions.iter().flatten().copied())
                .collect::<BTreeSet<_>>()
                .len()
        })
    }
    /// Network Lock is confirmed on and no tunnel carries traffic: apps have
    /// no internet. Never claimed from a saved preference.
    fn blocking_traffic(&self) -> bool {
        self.protection_known && self.guarded && self.since.is_none()
    }
    /// The status shown in the sidebar, Overview and tray. An idle retained
    /// lock is not just "Disconnected": traffic is blocked.
    fn display_status(&self) -> String {
        if self.blocking_traffic()
            && self.control.is_none()
            && self.retry_at.is_none()
            && self.status == "Disconnected"
        {
            "Blocking traffic".into()
        } else {
            self.status.clone()
        }
    }
    /// Overview's protection line: why status is unknown, or that traffic
    /// is blocked and how to restore it.
    fn protection_note(&self) -> Option<&'static str> {
        if !self.protection_known {
            Some(if self.helper_missing() {
                HELPER_MISSING
            } else {
                "Protection status is unavailable. No protection claim is made until the networking service confirms it."
            })
        } else if self.blocking_traffic() {
            Some("Network Lock is blocking traffic: apps have no internet until you connect or choose Turn off Network Lock. AirVPN sign-in remains allowed.")
        } else {
            None
        }
    }
    fn tray_snapshot(&self) -> tray::Snapshot {
        tray::Snapshot {
            status: self.display_status(),
            // A pending reconnect is cancellable too: offer Disconnect, not
            // an immediate Connect that skips the backoff.
            active: self.control.is_some() || self.retry_at.is_some(),
            signed_in: self.account.is_some(),
        }
    }
    /// The power button cancels/disconnects instead of connecting.
    fn power_cancels(&self) -> bool {
        self.control.is_some() || self.retry_at.is_some() || self.since.is_some()
    }
    /// Device names to choose from; None hides the setting (one device).
    fn device_choices(&self) -> Option<Vec<String>> {
        let a = self.account.as_ref()?;
        (a.devices.len() > 1).then(|| a.devices.iter().map(|d| d.name.clone()).collect())
    }
    /// The helper is not installed, so status queries cannot answer.
    fn helper_missing(&self) -> bool {
        #[cfg(test)]
        if let Some(available) = self.test_helper {
            return !available;
        }
        !self.preview && !wave_protocol::service::available()
    }
    /// Why the target matches no available server, naming it.
    fn no_server_reason(&self) -> String {
        match &self.prefs.target {
            Target::Best => {
                "No AirVPN server is available right now. Refresh the server list, then try again."
                    .into()
            }
            Target::Country(c) => format!(
                "No server in {} is available right now. Choose another country or Best server.",
                views::country(c)
            ),
            Target::Server(n) => {
                format!("{n} is not available right now. Choose another server or Best server.")
            }
        }
    }
    fn cancel_measurement(&mut self) {
        self.latency_cancel.store(true, Ordering::Relaxed);
        self.latency.cancel();
    }
    fn query_status(&mut self) {
        self.status_pending = true;
        let revision = self.network_revision;
        let tx = self.sender.clone();
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<wave_protocol::protection::Status> {
                let executable = std::env::current_exe()?.with_file_name("wave-vpn");
                // Read-only query: never install the helper or prompt for a password.
                let output = wave_protocol::process::output(
                    std::process::Command::new(executable)
                        .args(["service-client", "network-status"]),
                    None,
                    Duration::from_secs(8),
                )?;
                anyhow::ensure!(output.status.success(), "Networking status unavailable");
                for line in output.stdout.split(|b| *b == b'\n') {
                    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(line) {
                        if value["event"] == "network_status" {
                            return Ok(serde_json::from_value(value["status"].clone())?);
                        }
                    }
                }
                anyhow::bail!("Networking status unavailable")
            })()
            .map_err(|_| "Networking status unavailable".into());
            let _ = tx.send(Event::NetworkStatus(revision, result));
        });
    }
    fn refresh(&mut self, manual: bool) {
        if self.preview || self.refresh.running() || self.account.is_none() {
            return;
        }
        let epoch = self.account_epoch;
        self.refresh.start(Instant::now());
        // Manual refreshes fetch the account too; automatic ones only about
        // once a day, so device or subscription changes still arrive.
        let with_account = manual || schedule::account_due(self.account_checked, Instant::now());
        let login = self.login.clone();
        let mut password = self.session_password.clone();
        let tx = self.sender.clone();
        let timeout = self.prefs.options.number("http_timeout");
        let cancel = self.requests_cancel.clone();
        let store = self.bootstrap.clone();
        let pins = self.pins_required();
        std::thread::spawn(move || {
            let r = Client::with_timeout(timeout)
                .map(|c| {
                    c.remembering(store)
                        .cancellable(cancel)
                        .requiring_pins(pins)
                })
                .and_then(|c| {
                    if with_account {
                        c.login(&login, &password).map(|(a, m)| (Some(a), m))
                    } else {
                        c.manifest(&login, &password).map(|m| (None, m))
                    }
                })
                .map_err(|e| {
                    if e.is::<wave_protocol::AccountRejected>() {
                        wave_protocol::sign_in_error(&e)
                    } else {
                        "Unable to refresh AirVPN data".into()
                    }
                });
            password.zeroize();
            let _ = tx.send(Event::Refreshed(epoch, manual, Box::new(r)));
        });
    }
    fn transport_line(&mut self, line: String) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
            if v["event"] == "failure" {
                self.retryable = v["retryable"] == true;
                return;
            }
            if v["event"] == "checks" {
                if let Ok(results) = serde_json::from_value(v["results"].clone()) {
                    // Within one connection an observed failure stays shown.
                    self.checks.merge(&results);
                }
                return;
            }
            if v["event"] == "protection" {
                self.network_revision = self.network_revision.wrapping_add(1);
                // The worker applied the set it was started with.
                if self.control.is_some() {
                    if let Some(list) = self.connection_exemptions.take() {
                        self.applied_exemptions = Some(list);
                    }
                }
                self.protection_known = true;
                self.guarded = v["enabled"].as_bool().unwrap_or(false);
                self.protection.enabled = Some(self.guarded);
                if self.lock_change == Some(self.guarded) {
                    self.lock_change = None;
                    self.network_busy = false;
                    self.prefs.options.set("netlock", self.guarded.to_string());
                    self.draft.set("netlock", self.guarded.to_string());
                    self.persist();
                }
                return;
            }
            if v["event"] == "state" {
                self.status = match v["state"].as_str() {
                    Some("stalled") => "Stalled",
                    Some("checking") => "Checking connection…",
                    _ => "Connected",
                }
                .into();
                if self.status == "Stalled" {
                    self.checks = Default::default();
                }
                return;
            }
            if v["event"] == "stats" {
                let rx = v["rx"].as_u64().unwrap_or(0);
                let tx = v["tx"].as_u64().unwrap_or(0);
                let elapsed = self.last_stats.elapsed().as_secs_f64().max(0.01);
                self.rx_rate = rx.saturating_sub(self.rx) as f64 / elapsed;
                self.tx_rate = tx.saturating_sub(self.tx) as f64 / elapsed;
                self.rx = rx;
                self.tx = tx;
                self.last_stats = Instant::now();
                return;
            }
            if v["event"] == "connected" {
                self.connected();
                return;
            }
            if v["event"] == "lock" {
                self.persistent_lock = v["enabled"].as_bool().unwrap_or(false);
                self.lock_installed = v["installed"].as_bool().unwrap_or(false);
                return;
            }
        }
        if let Some(problem) = helper_problem(&line) {
            self.helper_problem = Some(problem);
        }
        if line.contains("Connected to WireGuard server") {
            self.connected();
        }
        if line.contains("handshake expired") {
            self.status = "Stalled".into();
        }
        let lower = line.to_lowercase();
        let level = if lower.contains("error") {
            Level::Error
        } else if lower.contains("warn") {
            Level::Warn
        } else {
            Level::Info
        };
        self.log(level, line);
    }
    fn connected(&mut self) {
        if self.since.is_none() {
            self.since = Some(Instant::now());
            self.connections += 1;
            self.rx = 0;
            self.tx = 0;
            self.last_stats = Instant::now();
        }
        self.status = "Connected".into();
        if let Some(s) = &self.active_server {
            self.prefs.last_server = s.name.clone();
        }
        self.persist();
    }
    fn save_account(&mut self) {
        let Some(storage) = self.storage else {
            return;
        };
        self.keyring_busy = true;
        let epoch = self.account_epoch;
        let tx = self.sender.clone();
        let account = wave_protocol::credentials::Credentials {
            login: self.login.clone(),
            password: self.session_password.clone(),
        };
        std::thread::spawn(move || {
            let result = storage.save(&preferences::directory(), &account)
                .map_err(|_| "Signed in, but your account could not be saved. Unlock your keyring and sign in again.".to_owned());
            let _ = tx.send(Event::AccountSaved(epoch, result));
        });
    }
    fn privileged(&mut self, action: &'static str) {
        self.network_revision = self.network_revision.wrapping_add(1);
        let input = wave_protocol::transport::LockInput {
            options: self.prefs.options.clone(),
            bootstrap: self.bootstrap.exemptions(),
        };
        #[cfg(test)]
        let hooked = self.test_helper.is_some();
        #[cfg(not(test))]
        let hooked = false;
        if self.preview && !hooked {
            self.error = Some("Privileged actions are disabled in preview mode".into());
            return;
        }
        self.network_busy = true;
        // Recover reuses the helper's own set; the lock actions apply this one.
        let generation = (action != "recover").then(|| {
            self.exemptions_generation += 1;
            self.exemptions_sent = Some((self.exemptions_generation, input.bootstrap.clone()));
            self.exemptions_generation
        });
        #[cfg(test)]
        if hooked {
            self.test_privileged.push((action, input));
            return;
        }
        let tx = self.sender.clone();
        std::thread::spawn(move || {
            let r = (|| -> anyhow::Result<String> {
                let executable = std::env::current_exe()?.with_file_name("wave-vpn");
                let mut child = wave_protocol::service::command(&executable, action)?
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(&serde_json::to_vec(&input)?)?;
                let result = child.wait_with_output()?;
                if !result.status.success() {
                    // Plain helper lines (e.g. a broker rejection) explain more
                    // than the client's generic exit message.
                    let stdout = String::from_utf8_lossy(&result.stdout);
                    let plain: Vec<&str> = stdout
                        .lines()
                        .filter(|l| !l.trim_start().starts_with('{'))
                        .collect();
                    anyhow::bail!(
                        "{}{}",
                        String::from_utf8_lossy(&result.stderr).trim(),
                        if plain.is_empty() {
                            String::new()
                        } else {
                            format!(": {}", plain.join("; "))
                        }
                    );
                }
                Ok(String::from_utf8_lossy(&result.stdout).into_owned())
            })();
            let _ = tx.send(Event::Notice(generation, r.map_err(|e| e.to_string())));
        });
    }
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_millis(100)).map(|_| Message::Tick),
            iced::window::resize_events().map(|(_, s)| Message::Resize(s)),
            iced::window::close_requests().map(Message::Close),
            iced::system::theme_changes().map(Message::SystemTheme),
        ])
    }
    fn theme(&self) -> Theme {
        let mode = if self.dirty() {
            self.draft.get("theme")
        } else {
            self.prefs.options.get("theme")
        };
        let dark = match std::env::var("AIRVPN_THEME")
            .ok()
            .as_deref()
            .unwrap_or(mode)
        {
            "light" => false,
            "dark" => true,
            _ => self.system_dark,
        };
        theme::palette(dark)
    }
    fn view(&self, _id: iced::window::Id) -> Element<'_, Message> {
        views::shell(self)
    }
    fn window_theme(&self, _id: iced::window::Id) -> Theme {
        self.theme()
    }
}
impl Drop for Wave {
    fn drop(&mut self) {
        self.password.zeroize();
        self.import_password.zeroize();
        self.session_password.zeroize();
        if let Some(control) = self.control.take() {
            control.lock().unwrap().input.take();
        }
    }
}
const KEEPING_SERVERS: &str = "Keeping the current server list";
const EXEMPTION_RETRY_MIN: Duration = Duration::from_secs(30);
const EXEMPTION_RETRY_MAX: Duration = Duration::from_secs(15 * 60);
const HELPER_OUTDATED: &str = "Network Lock could not follow AirVPN's updated bootstrap addresses. The installed networking helper may be older than this app: reinstall it with the installer (sudo bash scripts/install.sh install), then reopen Wave VPN. Wave retries automatically.";
const HELPER_MISSING: &str = "The networking helper isn't installed yet, so protection status is unknown. Wave sets it up with one administrator prompt the first time you connect or change Network Lock.";
const HELPER_SETUP_REFUSED: &str = "Wave needs one administrator authorization to install its networking helper, and it was declined or unavailable. Press Connect again and approve the system prompt, or run sudo wave-vpn service-install in a terminal.";
const HELPER_OTHER_USER: &str = "The networking helper is authorized for a different desktop user. To use Wave VPN from this account, run sudo wave-vpn service-install here.";
const HELPER_REFUSED: &str = "The networking helper refused this request. It may be older than this app, for example after a package upgrade while connected: disconnect, run sudo wave-vpn service-install, then reopen Wave VPN.";
/// Explain a helper refusal (missing authorization, other user, outdated
/// helper) found in a helper line or error, instead of a generic failure.
fn helper_problem(text: &str) -> Option<&'static str> {
    if text.contains("not authorized for Wave VPN networking") {
        Some(HELPER_OTHER_USER)
    } else if text.contains("Networking service setup was cancelled or failed")
        || text.contains("Networking service setup cancelled or timed out")
        || text.contains(wave_protocol::service::PKEXEC_MISSING)
    {
        Some(HELPER_SETUP_REFUSED)
    } else if helper_outdated(text) {
        Some(HELPER_REFUSED)
    } else {
        None
    }
}
/// Rejections an older helper gives for the newer lock input or action.
fn helper_outdated(error: &str) -> bool {
    [
        "Unsupported service action",
        "Unknown lock action",
        "unknown field",
        "rejected or could not complete",
    ]
    .iter()
    .any(|needle| error.contains(needle))
}
fn launch_transport(
    profile: TunnelProfile,
    options: Options,
    checks: Option<wave_protocol::checks::ProviderCheck>,
    bootstrap: Vec<Exemption>,
    control: Arc<Mutex<WorkerControl>>,
    tx: mpsc::Sender<Event>,
) -> anyhow::Result<()> {
    let executable = std::env::current_exe()?.with_file_name("wave-vpn");
    anyhow::ensure!(
        executable.is_file(),
        "Build/install wave-vpn beside the GUI"
    );
    let mut child =
        wave_protocol::service::command_cancellable(&executable, "transport-worker", || {
            control.lock().unwrap().cancelled
        })?
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut input = child.stdin.take().unwrap();
    let mut bytes = serde_json::to_vec(&wave_protocol::transport::Request {
        profile,
        options,
        checks,
        bootstrap,
    })?;
    bytes.push(b'\n');
    let result = input.write_all(&bytes);
    bytes.zeroize();
    result?;
    {
        let mut c = control.lock().unwrap();
        if c.cancelled {
            // Cancellation may arrive while pkexec is still prompting.
            input.write_all(if c.release {
                b"disconnect\n"
            } else {
                b"switch\n"
            })?;
        } else {
            c.input = Some(input);
        }
    }
    let err = child.stderr.take().unwrap();
    let errtx = tx.clone();
    let errors = std::thread::spawn(move || {
        for line in std::io::BufReader::new(err).lines().map_while(Result::ok) {
            let _ = errtx.send(Event::Line(line));
        }
    });
    for line in std::io::BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map_while(Result::ok)
    {
        let _ = tx.send(Event::Line(line));
    }
    let result = child.wait()?;
    let _ = errors.join();
    control.lock().unwrap().input.take();
    anyhow::ensure!(
        result.success(),
        "Connection stopped unexpectedly; inspect Logs before retrying"
    );
    Ok(())
}
fn window_settings() -> iced::window::Settings {
    iced::window::Settings {
        size: if std::env::var("WAVE_PREVIEW_SCENE").as_deref() == Ok("narrow") {
            iced::Size::new(640., 700.)
        } else {
            iced::Size::new(900., 600.)
        },
        min_size: Some(iced::Size::new(560., 440.)),
        exit_on_close_request: false,
        // Matches wave-vpn.desktop, so docks and task switchers show the app's
        // name and icon (Wayland app_id, X11 WM_CLASS).
        #[cfg(target_os = "linux")]
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: "wave-vpn".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
fn main() -> iced::Result {
    iced::daemon(Wave::boot, Wave::update, Wave::view)
        .title(|_app: &Wave, _id| {
            if std::env::var_os("WAVE_PREVIEW").is_some() {
                "NEW — Wave VPN parity".to_string()
            } else {
                "Wave VPN".to_string()
            }
        })
        .theme(Wave::window_theme)
        .subscription(Wave::subscription)
        .default_font(theme::FONT)
        .font(include_bytes!("../resources/fonts/Geist-Regular.ttf").as_slice())
        .font(include_bytes!("../resources/fonts/Geist-Medium.ttf").as_slice())
        .font(include_bytes!("../resources/fonts/Geist-SemiBold.ttf").as_slice())
        .font(include_bytes!("../resources/fonts/GeistMono-Regular.ttf").as_slice())
        .run()
}

#[cfg(test)]
mod safety_tests {
    use super::*;
    fn app() -> Wave {
        let mut app = Wave::new();
        app.preview = true;
        app.prefs.options = Options::default();
        app
    }
    #[test]
    fn disconnect_warnings_require_a_current_idle_status() {
        for worker_active in [false, true] {
            let mut app = app();
            app.control = Some(Arc::new(Mutex::new(WorkerControl::default())));
            let status = wave_protocol::protection::Status {
                enabled: Some(true),
                worker_active,
                recovery_needed: true,
                ..Default::default()
            };
            let old_revision = app.network_revision;
            app.sender
                .send(Event::NetworkStatus(old_revision, Ok(status.clone())))
                .unwrap();
            let _ = app.update(Message::Tick);
            assert!(app.idle_network_warning().is_none());
            let _ = app.update(Message::Disconnect);
            app.sender.send(Event::End(String::new())).unwrap();
            let _ = app.update(Message::Tick);
            assert_eq!(app.status, "Disconnected");
            assert!(app.idle_network_warning().is_none());

            // A firewall confirmation must not revive the stale helper flags.
            app.transport_line(r#"{"event":"protection","enabled":true}"#.into());
            app.sender
                .send(Event::NetworkStatus(old_revision, Ok(status.clone())))
                .unwrap();
            let _ = app.update(Message::Tick);
            assert!(app.idle_network_warning().is_none());

            // Actual remaining work is still shown after a fresh query.
            app.sender
                .send(Event::NetworkStatus(app.network_revision, Ok(status)))
                .unwrap();
            let _ = app.update(Message::Tick);
            assert!(app.idle_network_warning().is_some());
            // Cleanup can be confirmed even if firewall inspection failed.
            app.protection_known = false;
            app.protection.enabled = None;
            assert!(app.idle_network_warning().is_some());
            app.sender
                .send(Event::NetworkStatus(
                    app.network_revision,
                    Err("Service unavailable".into()),
                ))
                .unwrap();
            let _ = app.update(Message::Tick);
            assert!(app.idle_network_warning().is_none());
            app.sender
                .send(Event::NetworkStatus(
                    app.network_revision,
                    Ok(wave_protocol::protection::Status {
                        enabled: Some(false),
                        ..Default::default()
                    }),
                ))
                .unwrap();
            let _ = app.update(Message::Tick);
            assert!(app.idle_network_warning().is_none());
        }
    }
    #[test]
    fn retry_is_cancellable_and_failed_session_cannot_stay_connected() {
        let mut app = app();
        app.status = "Connected".into();
        app.since = Some(Instant::now());
        app.retryable = true;
        app.sender
            .send(Event::End("Connection ended".into()))
            .unwrap();
        let _ = app.update(Message::Tick);
        assert_eq!(app.status, "Reconnecting…");
        assert!(app.since.is_none() && app.retry_at.is_some());
        let _ = app.update(Message::Disconnect);
        assert!(app.retry_at.is_none());
        assert_eq!(app.status, "Disconnected");
    }
    #[test]
    fn setup_errors_do_not_loop_and_stale_queries_cannot_override_confirmed_lock() {
        let mut app = app();
        app.retryable = false;
        app.sender
            .send(Event::End("Helper setup cancelled".into()))
            .unwrap();
        let _ = app.update(Message::Tick);
        assert!(app.retry_at.is_none());
        app.transport_line(r#"{"event":"protection","enabled":true}"#.into());
        app.sender
            .send(Event::NetworkStatus(
                0,
                Ok(wave_protocol::protection::Status {
                    enabled: Some(false),
                    ..Default::default()
                }),
            ))
            .unwrap();
        let _ = app.update(Message::Tick);
        assert!(app.guarded && app.protection_known);
        app.sender
            .send(Event::NetworkStatus(
                app.network_revision,
                Err("Service unavailable".into()),
            ))
            .unwrap();
        let _ = app.update(Message::Tick);
        assert!(!app.protection_known && app.protection.enabled.is_none());
    }
    fn signed_in() -> Wave {
        use base64::Engine;
        let mut app = app();
        let key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
        app.account = Some(
            wave_protocol::parse_account(
                &format!(
                    r#"<user login="tester" wg_public_key="{key}"><keys><key name="laptop"
                    wg_private_key="{key}" wg_preshared="{key}" wg_ipv4="10.1.2.3/32"
                    wg_dns_ipv4="10.1.0.1"/></keys></user>"#
                ),
                "tester",
            )
            .unwrap(),
        );
        app.manifest = Some(
            wave_protocol::parse_manifest(include_str!(
                "../protocol-data/fixtures/eddie-manifest.xml"
            ))
            .unwrap(),
        );
        app
    }
    #[test]
    fn failed_refresh_keeps_the_cached_account_and_servers_and_backs_off() {
        let mut app = signed_in();
        let servers = app.manifest.as_ref().unwrap().servers.len();
        app.refresh.start(Instant::now());
        app.sender
            .send(Event::Refreshed(
                app.account_epoch,
                false,
                Box::new(Err("Unable to refresh AirVPN data".into())),
            ))
            .unwrap();
        let _ = app.update(Message::Tick);
        assert!(app.account.is_some());
        assert_eq!(app.manifest.as_ref().unwrap().servers.len(), servers);
        assert!(app.error.is_none(), "a background failure is only logged");
        assert_eq!(app.refresh.failures(), 1);
        assert!(app.refresh.next() >= Instant::now() + Duration::from_secs(200));
        assert!(!app.manual_refresh_allowed(), "manual retry waits a minute");
        // A stale epoch (after sign-out/sign-in) cannot touch the new session.
        app.sender
            .send(Event::Refreshed(
                app.account_epoch.wrapping_add(1),
                true,
                Box::new(Err("old".into())),
            ))
            .unwrap();
        let _ = app.update(Message::Tick);
        assert!(app.error.is_none() && app.refresh.failures() == 1);
        app.refresh.start(Instant::now());
        app.sender
            .send(Event::Refreshed(
                app.account_epoch,
                true,
                Box::new(Err("Unable to refresh AirVPN data".into())),
            ))
            .unwrap();
        let _ = app.update(Message::Tick);
        assert!(app.error.as_deref().is_some_and(|e| e.contains("Keeping")));
    }
    #[test]
    fn latency_results_from_cancelled_runs_are_ignored_and_tunnels_block_probes() {
        let mut app = signed_in();
        assert!(!app.probes_blocked());
        let old = app.latency.begin(Instant::now());
        app.cancel_measurement();
        let current = app.latency.begin(Instant::now());
        for (generation, ms) in [(old, 1), (current, 42)] {
            app.sender
                .send(Event::Pings(
                    generation,
                    latency::Outcome::Complete(
                        [("Achernar".into(), latency::Ping::Measured(ms))].into(),
                    ),
                ))
                .unwrap();
        }
        let _ = app.update(Message::Tick);
        assert_eq!(app.latency.get("Achernar"), latency::Ping::Measured(42));
        app.control = Some(Arc::new(Mutex::new(WorkerControl::default())));
        assert!(app.probes_blocked());
        app.control = None;
        app.retry_at = Some(Instant::now());
        assert!(app.probes_blocked());
        app.retry_at = None;
        app.guarded = true;
        assert!(app.probes_blocked(), "Network Lock blocks probes anyway");
    }
    #[test]
    fn sign_out_cancels_in_flight_requests_and_clears_latency() {
        let mut app = signed_in();
        let in_flight = app.requests_cancel.clone();
        app.latency
            .insert("Achernar".into(), latency::Ping::Measured(9));
        app.sender
            .send(Event::SignedOut(app.account_epoch, Ok(())))
            .unwrap();
        let _ = app.update(Message::Tick);
        assert!(in_flight.load(Ordering::Relaxed));
        assert!(!app.requests_cancel.load(Ordering::Relaxed));
        assert_eq!(app.latency.get("Achernar"), latency::Ping::Unknown);
        assert!(app.account.is_none() && app.manifest.is_none());
    }
    #[test]
    fn an_unavailable_check_is_not_blamed_for_a_later_disconnect() {
        let mut app = app();
        app.transport_line(
            r#"{"event":"checks","results":{"ipv4":"verified","ipv6":"unknown","dns":"unavailable"}}"#
                .into(),
        );
        app.retryable = true;
        app.sender
            .send(Event::End("WireGuard handshake expired".into()))
            .unwrap();
        let _ = app.update(Message::Tick);
        assert_eq!(app.error.as_deref(), Some("WireGuard handshake expired"));
        let wait = app
            .retry_at
            .unwrap()
            .saturating_duration_since(Instant::now());
        assert!(wait <= wave_protocol::lifecycle::retry_delay(0).mul_f64(1.2));
    }
}

#[cfg(test)]
#[path = "../tests/support/best_gui_cases.rs"]
mod best_gui_cases;
#[cfg(test)]
#[path = "../tests/support/fix_gui_cases.rs"]
mod fix_gui_cases;
#[cfg(test)]
#[path = "../tests/support/forum_gui_cases.rs"]
mod forum_gui_cases;
#[cfg(test)]
#[path = "../tests/support/staff_gui_cases.rs"]
mod staff_gui_cases;
