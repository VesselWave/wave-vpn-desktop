// SPDX-License-Identifier: GPL-3.0-or-later
// New application/controller. Only icons/fonts are copied from authored PR additions.
mod icons;
mod theme;

use iced::{
    widget::{button, column, container, pick_list, row, scrollable, text, text_input, toggler},
    Element, Length, Subscription, Task, Theme,
};
use icons::{icon, Icon};
use std::{
    collections::HashSet,
    io::{BufRead, Write},
    process::{ChildStdin, Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};
use theme::Ink;
use wave_protocol::{Account, Client, Manifest, Server, TunnelProfile};
use zeroize::Zeroize;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Overview,
    Servers,
    Logs,
    Settings,
}
#[derive(Clone)]
enum Message {
    Tick,
    Login(String),
    Password(String),
    SignIn,
    SignOut,
    Page(Tab),
    Search(String),
    Select(String),
    Device(String),
    Port(u16),
    Ipv6(bool),
    Dark(bool),
    Favorite(String),
    Connect,
    Disconnect,
    Clear,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Wave UI event")
    }
}
enum Event {
    Login(Box<Result<(Account, Manifest), String>>),
    Line(String),
    End(String),
}

#[derive(Default)]
struct WorkerControl {
    cancelled: bool,
    input: Option<ChildStdin>,
}
impl WorkerControl {
    fn stop(&mut self) {
        self.cancelled = true;
        self.input.take();
    }
}

struct Wave {
    login: String,
    password: String,
    busy: bool,
    account: Option<Account>,
    manifest: Option<Manifest>,
    tab: Tab,
    search: String,
    selected: String,
    device: String,
    port: u16,
    ipv6: bool,
    dark: bool,
    favorites: HashSet<String>,
    logs: Vec<String>,
    status: String,
    events: mpsc::Receiver<Event>,
    sender: mpsc::Sender<Event>,
    control: Option<Arc<Mutex<WorkerControl>>>,
}
impl Wave {
    fn new() -> Self {
        let (sender, events) = mpsc::channel();
        Self {
            login: String::new(),
            password: String::new(),
            busy: false,
            account: None,
            manifest: None,
            tab: Tab::Overview,
            search: String::new(),
            selected: String::new(),
            device: String::new(),
            port: 1637,
            ipv6: false,
            dark: true,
            favorites: HashSet::new(),
            logs: vec![],
            status: "Disconnected".into(),
            events,
            sender,
            control: None,
        }
    }
    fn log(&mut self, line: String) {
        self.logs.push(line);
        if self.logs.len() > 500 {
            self.logs.remove(0);
        }
    }
    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tick => {
                while let Ok(event) = self.events.try_recv() {
                    match event {
                        Event::Login(result) => {
                            self.busy = false;
                            match *result {
                                Ok((a, m)) => {
                                    self.device = a
                                        .devices
                                        .first()
                                        .map(|d| d.name.clone())
                                        .unwrap_or_default();
                                    self.selected =
                                        m.best().map(|s| s.name.clone()).unwrap_or_default();
                                    self.account = Some(a);
                                    self.manifest = Some(m);
                                    self.status = "Disconnected".into();
                                    self.log("Signed in to AirVPN".into());
                                }
                                Err(e) => {
                                    self.status = e.clone();
                                    self.log(e);
                                }
                            }
                        }
                        Event::Line(line) => {
                            if line.contains("Connected to WireGuard server") {
                                self.status = "Tunnel active".into();
                            }
                            if line.contains("handshake expired")
                                || line.contains("handshake failed")
                            {
                                self.status = "Reconnecting…".into();
                            }
                            self.log(line);
                        }
                        Event::End(message) => {
                            self.control = None;
                            self.status = message.clone();
                            self.log(message);
                        }
                    }
                }
            }
            Message::Login(value) => self.login = value,
            Message::Password(value) => {
                self.password.zeroize();
                self.password = value;
            }
            Message::SignIn if !self.busy && self.account.is_none() => {
                if self.login.trim().is_empty() || self.password.is_empty() {
                    return Task::none();
                }
                self.busy = true;
                self.status = "Signing in…".into();
                let login = self.login.trim().to_owned();
                let mut password = std::mem::take(&mut self.password);
                let tx = self.sender.clone();
                std::thread::spawn(move || {
                    let result = Client::new()
                        .and_then(|c| c.login(&login, &password))
                        .map_err(|_| "Sign-in failed. Check your account and network.".into());
                    password.zeroize();
                    let _ = tx.send(Event::Login(Box::new(result)));
                });
            }
            Message::SignOut if self.control.is_none() => {
                self.account = None;
                self.manifest = None;
                self.logs.clear();
                self.status = "Disconnected".into();
            }
            Message::Page(tab) => self.tab = tab,
            Message::Search(value) => self.search = value,
            Message::Select(name) if self.control.is_none() => self.selected = name,
            Message::Device(value) if self.control.is_none() => self.device = value,
            Message::Port(value) if self.control.is_none() => self.port = value,
            Message::Ipv6(value) if self.control.is_none() => self.ipv6 = value,
            Message::Dark(value) => self.dark = value,
            Message::Favorite(name) => {
                if !self.favorites.remove(&name) {
                    self.favorites.insert(name);
                }
            }
            Message::Clear => self.logs.clear(),
            Message::Connect if self.control.is_none() => {
                let profile = self.profile();
                match profile {
                    Ok(profile) => {
                        let control = Arc::new(Mutex::new(WorkerControl::default()));
                        self.control = Some(control.clone());
                        self.status = "Starting Hummingbird…".into();
                        let tx = self.sender.clone();
                        std::thread::spawn(move || {
                            let result = launch_transport(profile, control, tx.clone());
                            let text = match result { Ok(()) => "Disconnected".into(),
                                Err(_) => "Transport stopped. Check Hummingbird installation and networking.".into() };
                            let _ = tx.send(Event::End(text));
                        });
                    }
                    Err(e) => {
                        self.status = e.to_string();
                    }
                }
            }
            Message::Disconnect => {
                if let Some(control) = &self.control {
                    // Closing stdin also handles disconnect while pkexec is awaiting authentication.
                    control.lock().unwrap().stop();
                    self.status = "Disconnecting…".into();
                }
            }
            _ => (),
        }
        Task::none()
    }
    fn profile(&self) -> anyhow::Result<TunnelProfile> {
        use anyhow::Context;
        let account = self.account.as_ref().context("Sign in first")?;
        let m = self.manifest.as_ref().context("Load server list first")?;
        let s = m
            .servers
            .iter()
            .find(|s| s.name == self.selected)
            .context("Choose a server")?;
        let mode = m
            .modes
            .iter()
            .find(|mode| mode.port == self.port)
            .context("Choose an advertised port")?;
        account.profile(s, &self.device, mode, self.ipv6)
    }
    fn subscription(&self) -> Subscription<Message> {
        iced::time::every(Duration::from_millis(200)).map(|_| Message::Tick)
    }
    fn theme(&self) -> Theme {
        theme::palette(self.dark)
    }
    fn view(&self) -> Element<'_, Message> {
        if self.account.is_none() {
            let form = column![
                text("Wave VPN").size(34),
                text("Independent AirVPN client · GPLv3"),
                text_input("AirVPN username", &self.login).on_input(Message::Login),
                text_input("Password", &self.password)
                    .secure(true)
                    .on_input(Message::Password)
                    .on_submit(Message::SignIn),
                button(if self.busy {
                    "Signing in…"
                } else {
                    "Sign in"
                })
                .on_press_maybe((!self.busy).then_some(Message::SignIn)),
                text(&self.status),
                text("Credentials remain in memory for this session.").size(12)
            ]
            .spacing(16)
            .width(360);
            return container(form).center(Length::Fill).into();
        }
        let mut nav = column![text("Wave VPN").size(24)].spacing(12);
        for (tab, label, which) in [
            (Tab::Overview, "Overview", Icon::Power),
            (Tab::Servers, "Servers", Icon::Globe),
            (Tab::Logs, "Logs", Icon::Logs),
            (Tab::Settings, "Settings", Icon::Sliders),
        ] {
            nav = nav.push(
                button(
                    row![
                        icon(
                            which,
                            18.0,
                            if self.tab == tab {
                                Ink::Accent
                            } else {
                                Ink::Muted
                            }
                        ),
                        text(label)
                    ]
                    .spacing(12),
                )
                .on_press(Message::Page(tab))
                .width(Length::Fill),
            );
        }
        nav = nav
            .push(iced::widget::Space::new().height(Length::Fill))
            .push(text(&self.status).size(12))
            .push(text("Unofficial · WireGuard").size(12));
        let content = match self.tab {
            Tab::Overview => self.overview(),
            Tab::Servers => self.servers(),
            Tab::Logs => {
                let mut entries = column![row![
                    text("Logs").size(26),
                    button("Clear").on_press(Message::Clear)
                ]
                .spacing(24)]
                .spacing(8);
                for line in &self.logs {
                    entries = entries.push(text(line).font(theme::MONO).size(12));
                }
                scrollable(entries).into()
            }
            Tab::Settings => self.settings(),
        };
        row![
            container(nav)
                .width(196)
                .height(Length::Fill)
                .padding(20)
                .style(theme::panel),
            container(content)
                .padding(32)
                .width(Length::Fill)
                .height(Length::Fill)
        ]
        .into()
    }
    fn overview(&self) -> Element<'_, Message> {
        let selected = self
            .manifest
            .as_ref()
            .and_then(|m| m.servers.iter().find(|s| s.name == self.selected));
        let location = selected
            .map(|s| format!("{}, {}", s.location, s.country.to_uppercase()))
            .unwrap_or_default();
        let live = self.control.is_some();
        let body = column![
            text(&self.status).size(14),
            text(&self.selected).size(34),
            text(location),
            button(icon(Icon::Power, 48.0, Ink::Accent))
                .padding(28)
                .on_press(if live {
                    Message::Disconnect
                } else {
                    Message::Connect
                }),
            button("Choose a server").on_press(Message::Page(Tab::Servers)),
            text("Linux connections use official AirVPN Hummingbird with Network Lock enabled.")
                .size(13),
            text("Administrator authentication is required to start the tunnel.").size(12)
        ]
        .spacing(24)
        .align_x(iced::Alignment::Center)
        .max_width(560);
        container(body).center(Length::Fill).into()
    }
    fn servers(&self) -> Element<'_, Message> {
        let query = self.search.to_lowercase();
        let mut list = column![].spacing(6);
        if let Some(m) = &self.manifest {
            let mut servers: Vec<&Server> = m
                .servers
                .iter()
                .filter(|s| {
                    query.is_empty()
                        || format!("{} {} {}", s.name, s.country, s.location)
                            .to_lowercase()
                            .contains(&query)
                })
                .collect();
            servers.sort_by(|a, b| m.score(a).total_cmp(&m.score(b)));
            for s in servers {
                let selected = s.name == self.selected;
                let line = row![
                    button(if self.favorites.contains(&s.name) {
                        "★"
                    } else {
                        "☆"
                    })
                    .on_press(Message::Favorite(s.name.clone())),
                    button(column![
                        text(&s.name),
                        text(format!("{}, {}", s.location, s.country.to_uppercase())).size(12)
                    ])
                    .width(Length::Fill)
                    .on_press_maybe(
                        (s.available() && self.control.is_none())
                            .then(|| Message::Select(s.name.clone()))
                    ),
                    text(format!(
                        "{:.0}% · {} users{}",
                        s.load(),
                        s.users,
                        if selected { " · selected" } else { "" }
                    ))
                    .font(theme::MONO)
                    .size(12)
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center);
                list = list.push(container(line).padding(10).style(theme::panel));
            }
        }
        column![
            text("Servers").size(26),
            text_input("Search servers, cities, countries", &self.search).on_input(Message::Search),
            scrollable(list)
        ]
        .spacing(16)
        .into()
    }
    fn settings(&self) -> Element<'_, Message> {
        let account = self.account.as_ref().unwrap();
        let ports = self
            .manifest
            .as_ref()
            .map(|m| m.modes.iter().map(|mode| mode.port).collect::<Vec<_>>())
            .unwrap_or_default();
        let devices = account
            .devices
            .iter()
            .map(|d| d.name.clone())
            .collect::<Vec<_>>();
        column![text("Settings").size(26), text(format!("Signed in as {}", account.login)),
            text(format!("Plan ends {}",account.expiration)),
            text("Device"),pick_list(devices,Some(self.device.clone()),Message::Device),
            text("WireGuard UDP port"),pick_list(ports,Some(self.port),Message::Port),
            toggler(self.ipv6).label("Reach server over IPv6").on_toggle(Message::Ipv6),
            toggler(self.dark).label("Dark theme").on_toggle(Message::Dark),
            text("Network Lock is enforced by Hummingbird during the connection.\nClose or disconnect gracefully to restore networking.").size(13),
            button("Sign out").on_press_maybe(self.control.is_none().then_some(Message::SignOut)),
            text("Wave VPN 0.2-dev · GPLv3 · Not affiliated with AirVPN").size(12)]
            .spacing(16).max_width(620).into()
    }
}
impl Drop for Wave {
    fn drop(&mut self) {
        self.password.zeroize();
        if let Some(control) = self.control.take() {
            control.lock().unwrap().stop();
        }
    }
}

fn launch_transport(
    profile: TunnelProfile,
    control: Arc<Mutex<WorkerControl>>,
    tx: mpsc::Sender<Event>,
) -> anyhow::Result<()> {
    let executable = std::env::current_exe()?.with_file_name("wave-vpn");
    anyhow::ensure!(executable.is_file(), "Install wave-vpn beside wave-vpn-gui");
    let mut child = Command::new("/usr/bin/pkexec")
        .arg(executable)
        .arg("transport-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    let mut bytes = serde_json::to_vec(&profile)?;
    bytes.push(b'\n');
    let result = stdin.write_all(&bytes);
    bytes.zeroize();
    result?;
    {
        let mut controller = control.lock().unwrap();
        if !controller.cancelled {
            controller.input = Some(stdin);
        }
        // If already cancelled, dropping stdin gives the worker EOF as soon as pkexec starts it.
    }
    let err = child.stderr.take().unwrap();
    let err_tx = tx.clone();
    let stderr_thread = std::thread::spawn(move || {
        for line in std::io::BufReader::new(err).lines().map_while(Result::ok) {
            let _ = err_tx.send(Event::Line(line));
        }
    });
    for line in std::io::BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map_while(Result::ok)
    {
        let _ = tx.send(Event::Line(line));
    }
    let status = child.wait()?;
    let _ = stderr_thread.join();
    control.lock().unwrap().input.take();
    anyhow::ensure!(status.success(), "Transport worker failed");
    Ok(())
}
fn main() -> iced::Result {
    iced::application(Wave::new, Wave::update, Wave::view)
        .title("Wave VPN")
        .theme(Wave::theme)
        .subscription(Wave::subscription)
        .default_font(theme::FONT)
        .font(include_bytes!("../resources/fonts/Geist-Regular.ttf").as_slice())
        .font(include_bytes!("../resources/fonts/GeistMono-Regular.ttf").as_slice())
        .window_size((1024.0, 720.0))
        .run()
}
