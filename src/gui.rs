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
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
use wave_protocol::{
    options::{Options, Section},
    Account, Client, Manifest, Server, TunnelProfile,
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
    Refreshed(u64, Box<Result<(Account, Manifest), String>>),
    Imported(u64, Result<wave_protocol::eddie::Imported, String>),
    AccountSaved(u64, Result<(), String>),
    StorageLoaded(
        u64,
        wave_protocol::credentials::Storage,
        Result<Option<wave_protocol::credentials::Credentials>, String>,
    ),
    SignedOut(u64, Result<(), String>),
    Pings(BTreeMap<String, u32>),
    Line(String),
    End(String),
    Notice(Result<String, String>),
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
    refreshing: bool,
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
    pings: BTreeMap<String, u32>,
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
    last_refresh: Instant,
    capture_tick: usize,
    preview: bool,
    persistent_lock: bool,
    lock_installed: bool,
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
            refreshing: false,
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
            pings: BTreeMap::new(),
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
            last_refresh: Instant::now(),
            capture_tick: 0,
            preview: std::env::var_os("WAVE_PREVIEW").is_some(),
            persistent_lock: false,
            lock_installed: false,
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
            self.pings.get(&server.name).copied(),
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
                        Event::Login(epoch, result) | Event::Refreshed(epoch, result) => {
                            if epoch != self.account_epoch {
                                continue;
                            }
                            self.busy = false;
                            self.refreshing = false;
                            match *result {
                                Ok((a, m)) => {
                                    let first = self.account.is_none();
                                    if self.prefs.device.is_empty() {
                                        self.prefs.device = a.devices[0].name.clone();
                                    }
                                    self.account = Some(a);
                                    self.manifest = Some(m);
                                    self.last_refresh = Instant::now();
                                    self.error = None;
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
                                    self.measure();
                                    if first {
                                        self.save_account();
                                    }
                                    if first && self.prefs.options.on("autoconnect") {
                                        return self.update(Message::Connect);
                                    }
                                }
                                Err(e) => {
                                    self.error = Some(e.clone());
                                    self.log(Level::Error, e);
                                    if self.account.is_none() {
                                        self.session_password.zeroize();
                                    }
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
                                    self.refreshing = false;
                                    self.retry_at = None;
                                    self.session_server = None;
                                    self.account = None;
                                    self.manifest = None;
                                    self.login.zeroize();
                                    self.password.zeroize();
                                    self.session_password.zeroize();
                                    self.pings.clear();
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
                        Event::Pings(pings) => {
                            self.pings = pings;
                        }
                        Event::Line(line) => self.transport_line(line),
                        Event::End(message) => {
                            self.network_busy = false;
                            self.lock_change = None;
                            if self.quitting {
                                return iced::exit();
                            }
                            let cancelled = self
                                .control
                                .as_ref()
                                .is_some_and(|c| c.lock().unwrap().cancelled);
                            let retry = !message.is_empty()
                                && !cancelled
                                && self.prefs.options.on("reconnect");
                            if let Some(server) = &self.active_server {
                                self.penalties.insert(
                                    server.name.clone(),
                                    Instant::now()
                                        + Duration::from_secs(self.prefs.options.number("penalty")),
                                );
                            }
                            self.retry_at = retry.then(|| {
                                Instant::now()
                                    + Duration::from_secs(
                                        self.prefs.options.number("penalty").max(1),
                                    )
                            });
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
                            if !message.is_empty() {
                                self.error = Some(message.clone());
                                self.log(Level::Error, message);
                            } else {
                                self.log(Level::Info, "Disconnected".into());
                            }
                        }
                        Event::Notice(result) => {
                            self.network_busy = false;
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
                                    self.error = Some(e.clone());
                                    self.log(Level::Error, e);
                                }
                            }
                        }
                    }
                }
                if let Some(tray) = &mut self.tray {
                    tray.update(tray::Snapshot {
                        status: self.status.clone(),
                        active: self.control.is_some(),
                        signed_in: self.account.is_some(),
                    });
                }
                if self.window.is_none() && !self.tray.as_ref().is_some_and(|t| t.visible()) {
                    return self.update(Message::Show);
                }
                if self.preview {
                    return preview::tick(self);
                }
                if let Ok(text) = std::fs::read_to_string("/run/wave-vpn-lock-status.json") {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        self.persistent_lock = v["enabled"].as_bool().unwrap_or(false);
                        self.lock_installed = v["installed"].as_bool().unwrap_or(false);
                    }
                }
                if let Ok(text) = std::fs::read_to_string("/run/wave-vpn-protection.json") {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        self.guarded = v["enabled"].as_bool().unwrap_or(false);
                    }
                }
                if self.retry_at.is_some_and(|t| Instant::now() >= t) && self.control.is_none() {
                    self.retry_at = None;
                    return self.update(Message::Connect);
                }
                if self.account.is_some()
                    && !self.refreshing
                    && self.last_refresh.elapsed().as_secs() >= self.prefs.options.number("refresh")
                {
                    self.refresh();
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
                std::thread::spawn(move || {
                    let r = Client::with_timeout(timeout)
                        .and_then(|c| c.login(&login, &password))
                        .map_err(|_| "Sign-in failed. Check your account and network.".into());
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
            Message::Option(key, value) => self.draft.set(key, value),
            Message::Save => match self.draft.validate() {
                Ok(()) => {
                    self.prefs.options = self.draft.clone();
                    self.persist();
                    self.log(Level::Info, "Settings saved".into());
                }
                Err(e) => self.error = Some(e.to_string()),
            },
            Message::Refresh => self.refresh(),
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
                        std::thread::spawn(move || {
                            let result = launch_transport(profile, options, control, tx.clone());
                            let _ = tx.send(Event::End(
                                result.err().map(|e| e.to_string()).unwrap_or_default(),
                            ));
                        });
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
            Message::Disconnect => {
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
        let s = self
            .selected()
            .context("No available server matches this target")?;
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
    fn measure(&self) {
        if self.preview {
            return;
        }
        let servers = self
            .manifest
            .as_ref()
            .map(|m| m.servers.clone())
            .unwrap_or_default();
        let tx = self.sender.clone();
        let timeout = self.prefs.options.number("pinger_timeout");
        std::thread::spawn(move || {
            let mut pings = BTreeMap::new();
            for batch in servers.chunks(16) {
                let measured: Vec<_> = std::thread::scope(|scope| {
                    batch
                        .iter()
                        .map(|s| {
                            scope.spawn(move || {
                                let ip = s.entries.iter().find(|ip| ip.is_ipv4())?;
                                let start = Instant::now();
                                std::net::TcpStream::connect_timeout(
                                    &std::net::SocketAddr::new(*ip, 443),
                                    Duration::from_secs(timeout),
                                )
                                .ok()?;
                                Some((s.name.clone(), start.elapsed().as_millis().max(1) as u32))
                            })
                        })
                        .collect::<Vec<_>>()
                        .into_iter()
                        .filter_map(|h| h.join().ok().flatten())
                        .collect()
                });
                pings.extend(measured);
            }
            let _ = tx.send(Event::Pings(pings));
        });
    }
    fn refresh(&mut self) {
        if self.preview || self.refreshing || self.account.is_none() {
            return;
        }
        let epoch = self.account_epoch;
        self.refreshing = true;
        let login = self.login.clone();
        let mut password = self.session_password.clone();
        let tx = self.sender.clone();
        let timeout = self.prefs.options.number("http_timeout");
        std::thread::spawn(move || {
            let r = Client::with_timeout(timeout)
                .and_then(|c| c.login(&login, &password))
                .map_err(|_| "Unable to refresh AirVPN data".into());
            password.zeroize();
            let _ = tx.send(Event::Refreshed(epoch, Box::new(r)));
        });
    }
    fn transport_line(&mut self, line: String) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
            if v["event"] == "protection" {
                self.guarded = v["enabled"].as_bool().unwrap_or(false);
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
                self.status = if v["state"] == "stalled" {
                    "Stalled"
                } else {
                    "Connected"
                }
                .into();
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
        if self.preview {
            self.error = Some("Privileged actions are disabled in preview mode".into());
            return;
        }
        self.network_busy = true;
        let tx = self.sender.clone();
        let opts = self.prefs.options.clone();
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
                    .write_all(&serde_json::to_vec(&opts)?)?;
                let result = child.wait_with_output()?;
                anyhow::ensure!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                Ok(String::from_utf8_lossy(&result.stdout).into_owned())
            })();
            let _ = tx.send(Event::Notice(r.map_err(|e| e.to_string())));
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
fn launch_transport(
    profile: TunnelProfile,
    options: Options,
    control: Arc<Mutex<WorkerControl>>,
    tx: mpsc::Sender<Event>,
) -> anyhow::Result<()> {
    let executable = std::env::current_exe()?.with_file_name("wave-vpn");
    anyhow::ensure!(
        executable.is_file(),
        "Build/install wave-vpn beside the GUI"
    );
    let mut child = wave_protocol::service::command(&executable, "transport-worker")?
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut input = child.stdin.take().unwrap();
    let mut bytes = serde_json::to_vec(&wave_protocol::transport::Request { profile, options })?;
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
