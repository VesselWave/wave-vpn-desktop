// SPDX-License-Identifier: GPL-3.0-or-later
// New rendering code matching the old desktop's documented controls and visual hierarchy.
use super::*;
use iced::{
    widget::{
        button, column, container, pick_list, row, scrollable, text, text_input, toggler, tooltip,
        Space,
    },
    Alignment, Fill, Length,
};
use icons::{icon, Icon};
use theme::Ink;
use wave_protocol::latency::Ping;
fn line<'a>() -> Element<'a, Message> {
    container(Space::new())
        .width(Fill)
        .height(1)
        .style(theme::rule)
        .into()
}
fn link<'a>(label: &'a str, msg: Message) -> Element<'a, Message> {
    button(text(label).size(14))
        .style(theme::ghost)
        .on_press(msg)
        .padding([4, 6])
        .into()
}
fn target_label(t: &Target) -> String {
    match t {
        Target::Best => "Best server".into(),
        Target::Country(c) => country(c),
        Target::Server(n) => n.clone(),
    }
}
pub fn country(c: &str) -> String {
    match c.to_lowercase().as_str() {
        "nl" => "Netherlands",
        "de" => "Germany",
        "us" => "United States",
        "ca" => "Canada",
        "uk" | "gb" => "United Kingdom",
        "ch" => "Switzerland",
        "se" => "Sweden",
        "no" => "Norway",
        "fi" => "Finland",
        "fr" => "France",
        "be" => "Belgium",
        "at" => "Austria",
        "es" => "Spain",
        "it" => "Italy",
        "ie" => "Ireland",
        "sg" => "Singapore",
        "jp" => "Japan",
        "hk" => "Hong Kong",
        "ro" => "Romania",
        "bg" => "Bulgaria",
        "ee" => "Estonia",
        "lv" => "Latvia",
        "au" => "Australia",
        "br" => "Brazil",
        _ => return c.to_uppercase(),
    }
    .into()
}
pub fn shell(app: &Wave) -> Element<'_, Message> {
    let rail = app.width < 760.;
    let mut nav = column![].spacing(2);
    for (tab, label, glyph) in [
        (Tab::Overview, "Overview", Icon::Power),
        (Tab::Servers, "Servers", Icon::Globe),
        (Tab::Logs, "Logs", Icon::Logs),
        (Tab::Settings, "Settings", Icon::Sliders),
    ] {
        let glyph = icon(
            glyph,
            if rail { 18. } else { 16. },
            if app.tab == tab {
                Ink::Text
            } else {
                Ink::Muted
            },
        );
        let content: Element<_> = if rail {
            glyph
        } else {
            row![glyph, text(label).size(14).font(theme::MEDIUM)]
                .spacing(10)
                .align_y(Alignment::Center)
                .into()
        };
        let b = button(content)
            .padding([8, 10])
            .width(Fill)
            .on_press(Message::Page(tab))
            .style(theme::selected(app.tab == tab));
        if rail {
            nav = nav.push(tooltip(
                b,
                container(text(label).size(12))
                    .padding(6)
                    .style(theme::panel),
                tooltip::Position::Right,
            ));
        } else {
            nav = nav.push(b);
        }
    }
    let lock_on = app.guarded;
    let lock = button(
        row![
            icon(
                if lock_on { Icon::Lock } else { Icon::LockOpen },
                12.,
                Ink::Muted
            ),
            text(if !app.protection_known {
                "Lock status unknown"
            } else if lock_on {
                "Network lock on"
            } else {
                "Network lock off"
            })
            .size(12)
            .wrapping(iced::widget::text::Wrapping::None),
            icon(Icon::ChevronRight, 12., Ink::Muted)
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .padding([8, 10])
    .on_press(Message::Section(Section::Lock))
    .style(theme::ghost);
    let brand: Element<_> = if rail {
        icon(Icon::Power, 24., Ink::Accent)
    } else {
        row![
            text("Wave").size(15).font(theme::SEMIBOLD),
            text(" VPN")
                .size(15)
                .font(theme::SEMIBOLD)
                .style(theme::faint)
        ]
        .into()
    };
    let mut sidebar = column![
        container(brand).padding([4, 10]),
        Space::new().height(12),
        nav,
        Space::new().height(Fill)
    ]
    .spacing(2);
    if rail {
        sidebar = sidebar.push(
            button(icon(
                if lock_on { Icon::Lock } else { Icon::LockOpen },
                16.,
                Ink::Muted,
            ))
            .on_press(Message::Section(Section::Lock))
            .style(theme::ghost),
        );
    } else {
        let status = container(
            row![
                container(Space::new())
                    .width(6)
                    .height(6)
                    .style(move |t: &Theme| iced::widget::container::Style {
                        background: Some(
                            theme::color(
                                t,
                                if app.since.is_some() {
                                    Ink::Accent
                                } else {
                                    Ink::Faint
                                }
                            )
                            .into()
                        ),
                        border: iced::Border {
                            radius: theme::RADIUS_PILL.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                text(&app.status).size(13)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([4, 10]);
        sidebar = sidebar.push(column![status, lock].spacing(8));
    }
    let sidebar = container(sidebar)
        .padding(iced::Padding::from([16, 10]).bottom(24))
        .width(if rail { 56. } else { 196. })
        .height(Fill)
        .style(theme::sidebar);
    let content = if app.diagnostics.is_some() {
        diagnostics(app)
    } else {
        match app.tab {
            Tab::Overview => overview(app),
            Tab::Servers => servers(app),
            Tab::Logs => logs(app),
            Tab::Settings => settings(app),
        }
    };
    let mut main = column![].spacing(16);
    if app.prefs.options.on("show_errors") {
        if let Some(e) = &app.error {
            main = main.push(
                container(text(e).size(13).style(theme::danger))
                    .padding(12)
                    .width(Fill)
                    .style(theme::panel),
            );
        }
    }
    main = main.push(content);
    row![
        sidebar,
        container(Space::new())
            .width(1)
            .height(Fill)
            .style(theme::rule),
        container(main).padding(24).width(Fill).height(Fill)
    ]
    .into()
}
fn overview(app: &Wave) -> Element<'_, Message> {
    let live = app.control.is_some() || app.retry_at.is_some();
    let connected = app.since.is_some();
    let s = app.active_server.as_ref();
    let power = button(
        container(icon(
            Icon::Power,
            48.,
            if connected { Ink::OnAccent } else { Ink::Text },
        ))
        .center(136),
    )
    .padding(0)
    .on_press(if live || connected {
        Message::Disconnect
    } else {
        Message::Connect
    })
    .style(theme::power(connected));
    let chip = container(
        text(
            if let Some(since) = app.since.filter(|_| app.status == "Connected") {
                format!("• Connected · {}", uptime(since.elapsed().as_secs()))
            } else {
                format!("• {}", app.status)
            },
        )
        .size(13),
    )
    .padding([5, 12])
    .style(theme::status_chip(connected));
    let mut heading = column![text(if connected {
        s.map(|s| s.name.clone()).unwrap_or_default()
    } else {
        target_label(&app.prefs.target)
    })
    .size(36)
    .font(theme::SEMIBOLD)]
    .spacing(4)
    .align_x(Alignment::Center);
    if let Some(s) = s.filter(|_| connected) {
        heading = heading.push(
            text(format!("{}, {}", s.location, country(&s.country)))
                .size(16)
                .style(theme::muted),
        );
    } else {
        let choose = button(
            row![
                text(if app.prefs.target == Target::Best {
                    "Choose a server"
                } else {
                    "Change server"
                })
                .size(14),
                icon(Icon::ChevronRight, 14., Ink::Accent)
            ]
            .spacing(2)
            .align_y(Alignment::Center),
        )
        .padding(0)
        .style(theme::link)
        .on_press(Message::Page(Tab::Servers));
        let mut links = row![choose].spacing(16).align_y(Alignment::Center);
        if app.prefs.target != Target::Best {
            links = links.push(link("Use best server", Message::Select(Target::Best)));
        }
        heading = heading.push(links);
    }
    let status = column![chip, heading]
        .spacing(16)
        .align_x(Alignment::Center);
    let mut center = column![
        container(power).center_x(Fill),
        container(status).center_x(Fill)
    ]
    .spacing(24)
    .width(Fill);
    if live && !connected {
        center = center.push(text(&app.activity).size(12).style(theme::muted));
    }
    if !live && !connected {
        if let Some(a) = &app.account {
            if a.devices.len() > 1 {
                center = center.push(container(
                    row![
                        text("Device").size(14),
                        Space::new().width(Fill),
                        pick_list(
                            a.devices.iter().map(|d| d.name.clone()).collect::<Vec<_>>(),
                            Some(app.prefs.device.clone()),
                            Message::Device
                        )
                        .width(220)
                        .padding([7, 10])
                        .text_size(14)
                        .style(theme::picker)
                        .menu_style(theme::picker_menu)
                    ]
                    .align_y(Alignment::Center),
                ));
            }
        }
    }
    if app.control.is_some() || connected || app.checks.issue().is_some() {
        center = center.push(
            text(format!(
                "IPv4: {} · IPv6: {} · DNS: {}",
                app.checks.ipv4.label(),
                app.checks.ipv6.label(),
                app.checks.dns.label()
            ))
            .size(12)
            .style(theme::muted),
        );
    }
    if app.protection.worker_active && app.control.is_none() {
        center = center.push(text("Another Wave networking operation is active. Finish it before connecting or recovering.").size(13).style(theme::warning));
    }
    if app.protection.recovery_needed && app.control.is_none() {
        center = center.push(text("A previous Wave connection needs cleanup. Use Recover before reconnecting; Network Lock is retained.").size(13).style(theme::warning));
    }
    if !app.protection_known {
        center = center.push(text("Protection status is unavailable. No protection claim is made until the networking service confirms it.").size(12).style(theme::muted));
    } else if app.guarded && !connected {
        center = center.push(text("Internet is blocked by Network Lock. AirVPN sign-in remains allowed. Turn Network Lock off explicitly to restore ordinary internet.").size(12).style(theme::muted));
    }
    if connected {
        let cell = |label: &'static str, val: String| {
            column![
                text(label).size(12).style(theme::muted),
                text(val).size(16).font(theme::MONO)
            ]
            .spacing(6)
            .width(Fill)
        };
        let speeds = row![
            cell("↓ Download", amount(app.rx_rate, app, true)),
            cell("↑ Upload", amount(app.tx_rate, app, true))
        ]
        .spacing(16);
        let totals = row![
            cell("↓ Received", amount(app.rx as f64, app, false)),
            cell("↑ Sent", amount(app.tx as f64, app, false))
        ]
        .spacing(16);
        let compact = app.width - (if app.width < 760. { 56. } else { 196. }) - 48. < 520.;
        let stats: Element<_> = if compact {
            column![
                container(speeds).padding(16),
                line(),
                container(totals).padding(16)
            ]
            .into()
        } else {
            row![speeds, totals].spacing(16).padding(16).into()
        };
        center = center.push(container(stats).width(Fill).style(theme::panel));
    }
    let body = container(scrollable(
        container(container(center).max_width(560).width(Fill))
            .center_x(Fill)
            .width(Fill),
    ))
    .center_x(Fill)
    .center_y(Fill)
    .height(Fill);
    let footer = column![
        line(),
        row![
            text(if app.connections == 0 {
                String::new()
            } else {
                format!(
                    "{} {} this session",
                    app.connections,
                    if app.connections == 1 {
                        "connection"
                    } else {
                        "connections"
                    }
                )
            })
            .size(12)
            .style(theme::faint),
            Space::new().width(Fill),
            tooltip(
                button(
                    row![
                        icon(Icon::Wrench, 14., Ink::Muted),
                        text("Recover").size(12)
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center)
                )
                .padding([6, 10])
                .on_press(Message::Recover)
                .style(theme::ghost),
                container(
                    text("Clean up VPN state; keep Network Lock.")
                        .size(12)
                        .wrapping(iced::widget::text::Wrapping::None)
                )
                .max_width(300)
                .padding([6, 10])
                .style(theme::panel),
                tooltip::Position::Top,
            )
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(12)
    .width(Fill);
    column![
        body,
        container(container(footer).max_width(560).width(Fill)).center_x(Fill)
    ]
    .height(Fill)
    .into()
}
fn amount(mut n: f64, app: &Wave, rate: bool) -> String {
    let bits = app.prefs.options.get("units") == "bits";
    if bits {
        n *= 8.;
    }
    let iec = app.prefs.options.on("iec");
    let base = if iec { 1024. } else { 1000. };
    let mut i = 0;
    while n >= base && i < 4 {
        n /= base;
        i += 1;
    }
    format!(
        "{:.2} {}{}{}{}",
        n,
        ["", "K", "M", "G", "T"][i],
        if iec && i > 0 { "i" } else { "" },
        if bits { "b" } else { "B" },
        if rate { "/s" } else { "" }
    )
}
fn uptime(s: u64) -> String {
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}
fn servers(app: &Wave) -> Element<'_, Message> {
    let mut switches = row![].spacing(2);
    for (v, label) in [
        (ServerView::All, "All"),
        (ServerView::Favorites, "Favorites"),
        (ServerView::Countries, "Countries"),
    ] {
        switches = switches.push(
            button(text(label).size(13))
                .padding([5, 12])
                .style(theme::segment(v == app.server_view))
                .on_press(Message::ServerView(v)),
        );
    }
    let search = text_input("Search servers, countries, cities", &app.search)
        .on_input(Message::Search)
        .size(14)
        .padding([8, 12])
        .style(theme::input)
        .width(Fill);
    let count = app.manifest.as_ref().map(|m| m.servers.len()).unwrap_or(0);
    // Use the available content width after the navigation and page padding.
    let content_width = app.width - if app.width < 760. { 105. } else { 245. };
    let narrow = content_width < 600.;
    let stacked_controls = content_width < 640.;
    let controls = row![
        search,
        button(icon(Icon::Refresh, 16., Ink::Muted))
            .style(theme::ghost)
            .on_press_maybe(app.manual_refresh_allowed().then_some(Message::Refresh)),
        text(format!("{count} servers"))
            .size(13)
            .style(theme::muted),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let segments = container(switches).padding(3).style(theme::segments);
    let header: Element<_> = if stacked_controls {
        column![controls, segments].spacing(12).into()
    } else {
        row![controls, segments]
            .spacing(20)
            .align_y(Alignment::Center)
            .into()
    };
    let q = app.search.to_lowercase();
    let mut list: Vec<_> = app
        .manifest
        .as_ref()
        .map(|m| {
            m.servers
                .iter()
                .filter(|s| {
                    (app.server_view != ServerView::Favorites
                        || app.prefs.favorites.contains(&s.name))
                        && (q.is_empty()
                            || format!(
                                "{} {} {} {}",
                                s.name,
                                s.location,
                                s.country,
                                country(&s.country)
                            )
                            .to_lowercase()
                            .contains(&q))
                })
                .collect()
        })
        .unwrap_or_default();
    list.sort_by(|a, b| {
        let order = match app.sort {
            Sort::Name => a.name.cmp(&b.name),
            Sort::Country => a.country.cmp(&b.country).then(a.location.cmp(&b.location)),
            Sort::Load => a.load().total_cmp(&b.load()),
            Sort::Users => a.users.cmp(&b.users),
            Sort::Ping => app
                .latency
                .get(&a.name)
                .order()
                .cmp(&app.latency.get(&b.name).order()),
            Sort::Score => app
                .server_score(a)
                .unwrap_or(u32::MAX)
                .cmp(&app.server_score(b).unwrap_or(u32::MAX))
                .then(a.name.cmp(&b.name)),
        };
        if app.ascending {
            order
        } else {
            order.reverse()
        }
    });
    let mut rows = column![].spacing(2);
    let mut table_header = None;
    if app.server_view == ServerView::Countries {
        let mut groups: BTreeMap<&str, Vec<&Server>> = BTreeMap::new();
        for s in list {
            groups.entry(&s.country).or_default().push(s);
        }
        for (code, servers) in groups {
            let load = servers.iter().map(|s| s.load()).sum::<f64>() / servers.len() as f64;
            rows = rows.push(
                button(
                    row![
                        text(code.to_uppercase()).font(theme::MONO).width(40),
                        column![
                            text(country(code)).size(14).font(theme::MEDIUM),
                            text(format!("{} servers", servers.len()))
                                .size(12)
                                .style(theme::muted)
                        ]
                        .width(Fill),
                        text(format!("{load:.0}%")).font(theme::MONO).size(13)
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                )
                .width(Fill)
                .padding([14, 12])
                .style(theme::selected(
                    app.prefs.target == Target::Country(code.into()),
                ))
                .on_press(Message::Select(Target::Country(code.into()))),
            );
        }
    } else {
        let mut headings = row![].spacing(8);
        for (sort, label, width) in [
            (Sort::Name, "Server", 180.),
            (Sort::Country, "Location", 0.),
            (Sort::Load, "Load", 104.),
            (Sort::Users, "Users", 50.),
            (Sort::Ping, "Ping", 64.),
            (Sort::Score, "Score", 56.),
        ] {
            if narrow && matches!(sort, Sort::Country | Sort::Users | Sort::Score) {
                continue;
            }
            headings = headings.push(
                container(
                    button(
                        row![
                            text(label)
                                .size(12)
                                .wrapping(iced::widget::text::Wrapping::None),
                            if sort == app.sort {
                                icon(
                                    if app.ascending {
                                        Icon::ChevronUp
                                    } else {
                                        Icon::ChevronDown
                                    },
                                    12.,
                                    Ink::Muted,
                                )
                            } else {
                                Space::new().width(12).into()
                            }
                        ]
                        .spacing(4)
                        .align_y(Alignment::Center),
                    )
                    .padding([6, 0])
                    .on_press(Message::Sort(sort))
                    .style(theme::column_heading),
                )
                .width(if matches!(sort, Sort::Name | Sort::Country) {
                    Length::Fill
                } else {
                    Length::Fixed(width)
                }),
            );
        }
        table_header = Some(column![
            container(row![
                Space::new().width(46),
                container(headings)
                    .padding(iced::Padding::default().right(14))
                    .width(Fill)
            ])
            .padding(iced::Padding::default().right(12)),
            line()
        ]);
        if app.server_view == ServerView::All && q.is_empty() {
            rows = rows.push(
                button(
                    row![
                        container(icon(Icon::Zap, 16., Ink::Accent)).center_x(46),
                        container(text("Best server").size(14).font(theme::MEDIUM))
                            .padding(iced::Padding::default().right(14))
                            .width(Fill),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press(Message::Select(Target::Best))
                .style(theme::server_row(app.prefs.target == Target::Best))
                .width(Fill)
                .padding([8, 0]),
            );
        }
        for s in list {
            let favorite = button(
                container(icon(
                    if app.prefs.favorites.contains(&s.name) {
                        Icon::StarFilled
                    } else {
                        Icon::Star
                    },
                    16.,
                    if app.prefs.favorites.contains(&s.name) {
                        Ink::Warning
                    } else {
                        Ink::Faint
                    },
                ))
                .center_x(46)
                .center_y(19),
            )
            .padding(0)
            .style(|_, _| button::Style::default())
            .on_press(Message::Favorite(s.name.clone()));
            let name: Element<_> = if narrow {
                column![
                    text(&s.name).size(14).font(theme::MEDIUM),
                    text(format!("{}, {}", s.location, s.country.to_uppercase()))
                        .size(12)
                        .style(theme::muted)
                ]
                .width(Fill)
                .into()
            } else {
                text(&s.name)
                    .size(14)
                    .font(theme::MEDIUM)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .height(19)
                    .width(Fill)
                    .into()
            };
            let mut cells = row![name].spacing(8).align_y(Alignment::Center);
            if !narrow {
                let location = format!("{}, {}", s.location, s.country.to_uppercase());
                // Keep cities to one row; the tooltip always exposes the full location.
                let location_width = (content_width - 386.) / 2.;
                let limit = (location_width / 6.5).max(4.) as usize;
                let label = if location.chars().count() > limit {
                    format!("{}…", location.chars().take(limit - 1).collect::<String>())
                } else {
                    location.clone()
                };
                cells = cells.push(
                    container(tooltip(
                        text(label)
                            .width(Fill)
                            .size(12)
                            .height(19)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .style(theme::muted),
                        container(text(location).size(12))
                            .padding([6, 10])
                            .style(theme::panel),
                        tooltip::Position::Top,
                    ))
                    .width(Fill),
                );
            }
            let load = s.load().clamp(0., 100.);
            let role = if load < 50. {
                Ink::Success
            } else if load < 80. {
                Ink::Warning
            } else {
                Ink::Danger
            };
            let bar = container(row![
                container(Space::new())
                    .width(48. * load as f32 / 100.)
                    .height(4)
                    .style(theme::meter(role)),
                Space::new().width(48. * (100. - load) as f32 / 100.)
            ])
            .width(48)
            .height(4)
            .style(theme::meter(Ink::Faint));
            cells = cells.push(
                row![bar, text(format!("{load:.0}%")).font(theme::MONO).size(13)]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .width(104),
            );
            if !narrow {
                cells = cells.push(
                    text(s.users.to_string())
                        .width(50)
                        .font(theme::MONO)
                        .size(13),
                );
            }
            cells = cells.push(
                text(match app.latency.get(&s.name) {
                    Ping::Measured(n) => format!("{n} ms"),
                    Ping::Unreachable => "Timeout".into(),
                    Ping::Unknown => "—".into(),
                })
                .width(64)
                .font(theme::MONO)
                .style(move |t: &Theme| iced::widget::text::Style {
                    color: Some(theme::color(
                        t,
                        match app.latency.get(&s.name) {
                            Ping::Measured(n) if n < 60 => Ink::Success,
                            Ping::Measured(n) if n < 150 => Ink::Warning,
                            Ping::Measured(_) | Ping::Unreachable => Ink::Danger,
                            Ping::Unknown => Ink::Faint,
                        },
                    )),
                })
                .size(13),
            );
            if !narrow {
                cells = cells.push(
                    text(
                        app.server_score(s)
                            .map(|score| score.to_string())
                            .unwrap_or_else(|| "—".into()),
                    )
                    .width(56)
                    .font(theme::MONO)
                    .size(13),
                );
            }
            let select = button(
                row![
                    favorite,
                    container(cells)
                        .padding(iced::Padding::default().right(14))
                        .width(Fill)
                ]
                .align_y(Alignment::Center),
            )
            .width(Fill)
            .padding([8, 0])
            .on_press_maybe(
                s.available()
                    .then(|| Message::Select(Target::Server(s.name.clone()))),
            )
            .style(theme::server_row(
                app.prefs.target == Target::Server(s.name.clone()),
            ));
            rows = rows.push(select);
        }
    }
    if count == 0 {
        rows = rows.push(
            container(
                column![
                    text("No servers loaded").size(16).font(theme::MEDIUM),
                    text("Sign in and refresh to fetch the server list.")
                        .size(14)
                        .style(theme::muted),
                    link("Fetch server list", Message::Refresh)
                ]
                .spacing(8),
            )
            .padding(32),
        );
    }
    let connect = button(text(format!("Connect to {}", target_label(&app.prefs.target))).size(14))
        .on_press(Message::Connect)
        .padding([10, 20])
        .style(theme::primary);
    let mut table = column![].spacing(4).height(Fill);
    if let Some(headings) = table_header {
        table = table.push(headings);
    }
    table = table
        .push(scrollable(container(rows).padding(iced::Padding::default().right(12))).height(Fill));
    column![
        header,
        table,
        line(),
        row![
            text(target_label(&app.prefs.target))
                .size(13)
                .style(theme::muted),
            Space::new().width(Fill),
            connect
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(20)
    .height(Fill)
    .into()
}
fn logs(app: &Wave) -> Element<'_, Message> {
    let mut filters = row![].spacing(2);
    for level in [Level::Debug, Level::Info, Level::Warn, Level::Error] {
        filters = filters.push(
            button(text(level.label()).size(12))
                .style(theme::segment(app.filters.contains(&(level as u8))))
                .on_press(Message::Filter(level))
                .padding([6, 12]),
        );
    }
    let mut rows = column![].spacing(4);
    let mut visible = 0;
    for l in &app.logs {
        if !app.filters.contains(&(l.level as u8)) {
            continue;
        }
        visible += 1;
        rows = rows.push(
            row![
                text(&l.time)
                    .font(theme::MONO)
                    .size(12)
                    .width(64)
                    .style(theme::faint),
                text(l.level.label())
                    .font(theme::MONO)
                    .size(12)
                    .width(48)
                    .style(match l.level {
                        Level::Error => theme::danger,
                        Level::Warn => theme::warning,
                        _ => theme::faint,
                    }),
                text(&l.text).font(theme::MONO).size(12)
            ]
            .spacing(8),
        );
    }
    if visible == 0 {
        rows = rows.push(
            container(
                column![
                    text("No log entries").size(14).font(theme::MEDIUM),
                    text(if app.logs.is_empty() {
                        "Connection activity will appear here."
                    } else {
                        "Entries exist but are hidden by the level filters."
                    })
                    .size(13)
                    .style(theme::muted)
                ]
                .spacing(8),
            )
            .padding(24),
        );
    }
    column![
        row![
            filters,
            Space::new().width(Fill),
            link("Support report", Message::Diagnostics),
            link("Clear", Message::Clear)
        ]
        .align_y(Alignment::Center),
        container(scrollable(rows).height(Fill))
            .padding(12)
            .width(Fill)
            .height(Fill)
            .style(theme::panel)
    ]
    .spacing(20)
    .height(Fill)
    .into()
}
fn setting_row<'a>(
    label: &'a str,
    hint: &'a str,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let mut left = column![text(label).size(14)].spacing(2).width(Fill);
    if !hint.is_empty() {
        left = left.push(text(hint).size(12).style(theme::muted));
    }
    row![left, control]
        .spacing(16)
        .padding([11, 16])
        .align_y(Alignment::Center)
        .into()
}
fn group<'a>(title: &'a str, rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut panel = column![].spacing(0);
    for (i, r) in rows.into_iter().enumerate() {
        if i > 0 {
            panel = panel.push(line());
        }
        panel = panel.push(r);
    }
    column![
        text(title).size(14).font(theme::SEMIBOLD),
        container(panel).width(Fill).style(theme::panel)
    ]
    .spacing(8)
    .into()
}
fn settings(app: &Wave) -> Element<'_, Message> {
    use wave_protocol::options::{Input, FIELDS};
    let mut tabs = row![].spacing(2);
    for section in Section::ALL {
        tabs = tabs.push(
            button(text(section.label()).size(if app.width < 760. { 11 } else { 13 }))
                .padding([6, if app.width < 760. { 6 } else { 12 }])
                .style(theme::selected(app.section == section))
                .on_press(Message::Section(section)),
        );
    }
    let mut body = column![].spacing(24);
    if app.section == Section::General {
        let mut rows = vec![setting_row(
            "Credentials",
            if app.account.is_some() {
                ""
            } else {
                "Sign in with your AirVPN account"
            },
            if app.account.is_some() {
                row![
                    text("Configured").size(14).style(theme::accent),
                    button(text("Sign out").size(12))
                        .on_press_maybe(
                            (app.control.is_none() && !app.keyring_busy)
                                .then_some(Message::SignOut)
                        )
                        .style(theme::ghost)
                ]
                .spacing(10)
                .align_y(Alignment::Center)
                .into()
            } else {
                text("Not configured").size(14).style(theme::muted).into()
            },
        )];
        if app.account.is_none() {
            rows.push(
                column![
                    text_input("AirVPN username", &app.login)
                        .on_input(Message::Login)
                        .padding(8)
                        .style(theme::input),
                    text_input("Password", &app.password)
                        .secure(true)
                        .on_input(Message::Password)
                        .padding(8)
                        .style(theme::input)
                        .on_submit(Message::SignIn),
                    button(if app.busy || app.keyring_busy {
                        "Signing in…"
                    } else {
                        "Sign in"
                    })
                    .on_press_maybe((!app.busy && !app.keyring_busy).then_some(Message::SignIn))
                    .style(theme::primary)
                ]
                .spacing(8)
                .padding(16)
                .into(),
            );
        }

        if app.storage == Some(wave_protocol::credentials::Storage::Plaintext) {
            rows.push(container(text(if app.account.is_some() {
                "Secure storage is unavailable. Your password is saved in plaintext on this computer."
            } else {
                "Secure storage is unavailable. Signing in will save your password in plaintext on this computer."
            }).size(12).style(theme::warning)).padding(16).into());
        }
        body = body.push(group("Account", rows));
        if app.account.is_none() {
            body = body.push(group(
                "Import Eddie profile",
                vec![column![
                    text("Import your account details for review, then sign in.")
                        .size(12)
                        .style(theme::muted),
                    text_input("Absolute path to Eddie profile", &app.import_path)
                        .on_input(Message::ImportPath)
                        .padding(8)
                        .style(theme::input),
                    text_input(
                        "Profile password (if encrypted with your password)",
                        &app.import_password
                    )
                    .secure(true)
                    .on_input(Message::ImportPassword)
                    .padding(8)
                    .style(theme::input),
                    button(if app.importing {
                        "Importing…"
                    } else {
                        "Import for review"
                    })
                    .on_press_maybe(
                        (!app.importing && !app.busy && !app.import_path.is_empty())
                            .then_some(Message::ImportEddie)
                    )
                    .style(theme::primary)
                ]
                .spacing(8)
                .padding(16)
                .into()],
            ));
        }
    }
    let mut group_name = "";
    let mut group_rows = vec![];
    for field in FIELDS
        .iter()
        .filter(|f| f.section == app.section && wave_protocol::options::desktop_field(f.key))
    {
        if field.group != group_name && !group_rows.is_empty() {
            body = body.push(group(group_name, std::mem::take(&mut group_rows)));
        }
        group_name = field.group;
        let value = app.draft.get(field.key);
        let key = field.key;
        let control: Element<_> = match field.input {
            Input::Toggle => toggler(value == "true")
                .size(20)
                .style(theme::toggle)
                .on_toggle(move |v| Message::Option(key, v.to_string()))
                .into(),
            Input::Choice(choices) => pick_list(choices.to_vec(), Some(value), move |s| {
                Message::Option(key, s.into())
            })
            .padding([7, 10])
            .text_size(14)
            .width(160)
            .style(theme::picker)
            .menu_style(theme::picker_menu)
            .into(),
            Input::Text | Input::Number => text_input(
                if matches!(field.input, Input::Number) {
                    field.default
                } else {
                    ""
                },
                value,
            )
            .on_input(move |v| Message::Option(key, v))
            .padding([7, 10])
            .size(14)
            .style(theme::input)
            .width(if matches!(field.input, Input::Number) {
                100
            } else if app.width < 760. {
                160
            } else {
                240
            })
            .into(),
        };
        group_rows.push(setting_row(field.label, field.hint, control));
    }
    if !group_rows.is_empty() {
        body = body.push(group(group_name, group_rows));
    }
    if app.section == Section::General {
        body = body.push(group(
            "About",
            vec![
                setting_row(
                    "Version",
                    "",
                    text(env!("CARGO_PKG_VERSION"))
                        .font(theme::MONO)
                        .size(13)
                        .into(),
                ),
                container(
                    text(
                        "Independent client for AirVPN, not made by AirVPN. WireGuard only. GPLv3.",
                    )
                    .size(13)
                    .style(theme::muted),
                )
                .padding(16)
                .into(),
            ],
        ));
    }
    if app.section == Section::Lock {
        body = body.push(text(if app.persistent_lock { "Boot-persistent Network Lock is enabled." } else { "Session Network Lock survives app exit, but does not survive reboot. The networking helper does not enable boot protection." }).size(12).style(theme::muted));
        let available = !app.network_busy
            && !app.preview
            && app
                .control
                .as_ref()
                .is_none_or(|control| control.lock().unwrap().input.is_some());
        body = body.push(group(
            "Network Lock",
            vec![setting_row(
                "Block traffic outside VPN",
                if app.network_busy {
                    "Updating…"
                } else if !app.protection_known {
                    "Status unknown. Enable protection or explicitly disable it; a saved preference is not confirmation."
                } else if app.guarded {
                    "Stays on when you disconnect. Turn off to restore internet access."
                } else {
                    "Off. Internet access is available without the VPN."
                },
                if app.protection_known {
                    toggler(app.guarded).size(20).style(theme::toggle)
                        .on_toggle_maybe(available.then_some(|enabled| Message::LockAction(if enabled { "lock-enable" } else { "lock-disable" }))).into()
                } else {
                    row![button(text("Enable").size(12)).style(theme::ghost).on_press_maybe(available.then_some(Message::LockAction("lock-enable"))),
                        button(text("Disable").size(12)).style(theme::ghost).on_press_maybe(available.then_some(Message::LockAction("lock-disable")))].spacing(4).into()
                },
            )],
        ));
    }
    column![
        container(tabs).padding(3).style(theme::segments),
        scrollable(container(body).padding(iced::Padding {
            right: 8.,
            ..Default::default()
        }))
        .style(theme::scroll)
        .height(Fill),
        line(),
        row![
            text(if app.dirty() {
                "Unsaved changes"
            } else {
                "All changes saved"
            })
            .size(13)
            .style(theme::muted),
            Space::new().width(Fill),
            button(text("Save").size(14).font(theme::MEDIUM))
                .padding([8, 20])
                .on_press_maybe(app.dirty().then_some(Message::Save))
                .style(theme::primary)
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(20)
    .height(Fill)
    .into()
}

fn diagnostics(app: &Wave) -> Element<'_, Message> {
    column![
        row![text("Support report").size(24).font(theme::SEMIBOLD), Space::new().width(Fill), link("Back", Message::DiagnosticsClose)].align_y(Alignment::Center),
        text("Preview before sharing. Accounts, keys, addresses and raw logs are omitted. Nothing is uploaded.").size(13).style(theme::muted),
        scrollable(container(text(app.diagnostics.as_deref().unwrap_or("")).size(12).font(theme::MONO)).padding(12)).height(Fill),
        text_input("New report filename", &app.diagnostics_path).on_input(Message::DiagnosticsPath).size(13),
        row![link("Copy report", Message::DiagnosticsCopy), link("Save report", Message::DiagnosticsSave)].spacing(12),
    ].spacing(12).height(Fill).into()
}
