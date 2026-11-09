// SPDX-License-Identifier: GPL-3.0-or-later
// Independently implemented styles matching VesselWave's authored visual design.
use iced::widget::{button, container, scrollable, svg, text_input, toggler};
use iced::{Background, Border, Color, Font, Shadow, Theme};
pub const FONT: Font = Font::with_name("Geist");
pub const MEDIUM: Font = Font {
    weight: iced::font::Weight::Medium,
    ..FONT
};
pub const SEMIBOLD: Font = Font {
    weight: iced::font::Weight::Semibold,
    ..FONT
};
pub const MONO: Font = Font::with_name("Geist Mono");
pub const ACCENT: Color = Color::from_rgb8(47, 107, 255);
// Shared VesselWave corner scale (same in the Android app, Wave Remote and both sites):
// 8 rows/inputs/menus, 12 cards/panels, 16 sheets/dialogs/tiles, 24 site cards,
// pill for buttons/chips/segments/badges, circle for round icon actions.
pub const RADIUS_SM: f32 = 8.0;
pub const RADIUS_MD: f32 = 12.0;
pub const RADIUS_PILL: f32 = 999.0;
#[derive(Clone, Copy)]
pub struct Colors {
    pub bg: Color,
    pub sidebar: Color,
    pub surface: Color,
    pub border: Color,
    pub muted: Color,
    pub faint: Color,
    pub text: Color,
    pub hover: Color,
}
pub fn colors(t: &Theme) -> Colors {
    if t.extended_palette().is_dark {
        Colors {
            bg: Color::from_rgb8(9, 11, 16),
            sidebar: Color::from_rgb8(15, 17, 23),
            surface: Color::from_rgb8(18, 21, 28),
            border: Color::from_rgb8(37, 42, 53),
            muted: Color::from_rgb8(159, 166, 181),
            faint: Color::from_rgb8(106, 113, 130),
            text: Color::from_rgb8(236, 238, 243),
            hover: Color::from_rgb8(34, 39, 49),
        }
    } else {
        Colors {
            bg: Color::WHITE,
            sidebar: Color::from_rgb8(247, 247, 247),
            surface: Color::from_rgb8(250, 250, 250),
            border: Color::from_rgb8(230, 230, 230),
            muted: Color::from_rgb8(95, 95, 95),
            faint: Color::from_rgb8(163, 163, 163),
            text: Color::from_rgb8(10, 10, 10),
            hover: Color::from_rgb8(232, 232, 232),
        }
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum Ink {
    Text,
    Muted,
    Faint,
    Accent,
    OnAccent,
    Success,
    Warning,
    Danger,
}
pub fn color(t: &Theme, role: Ink) -> Color {
    let c = colors(t);
    match role {
        Ink::Text => c.text,
        Ink::Muted => c.muted,
        Ink::Faint => c.faint,
        Ink::OnAccent => Color::WHITE,
        // Text roles mirror the Android tokens (accentFg/good/warning/danger);
        // the light values keep state text readable on light surfaces.
        role => {
            let dark = t.extended_palette().is_dark;
            let (r, g, b) = match (role, dark) {
                (Ink::Accent, true) => (110, 155, 255),
                (Ink::Accent, false) => (31, 87, 230),
                (Ink::Success, true) => (63, 207, 142),
                (Ink::Success, false) => (22, 163, 74),
                (Ink::Warning, true) => (245, 165, 36),
                (Ink::Warning, false) => (180, 83, 9),
                // Danger.
                (_, true) => (255, 92, 92),
                (_, false) => (217, 45, 32),
            };
            Color::from_rgb8(r, g, b)
        }
    }
}
pub fn ink(role: Ink) -> impl Fn(&Theme, svg::Status) -> svg::Style {
    move |t, _| svg::Style {
        color: Some(color(t, role)),
    }
}
pub fn palette(dark: bool) -> Theme {
    let base = if dark { Theme::Dark } else { Theme::Light };
    let c = colors(&base);
    Theme::custom(
        "Wave",
        iced::theme::Palette {
            background: c.bg,
            text: c.text,
            primary: ACCENT,
            success: Color::from_rgb8(63, 207, 142),
            warning: Color::from_rgb8(245, 165, 36),
            danger: Color::from_rgb8(255, 92, 92),
        },
    )
}
pub fn panel(t: &Theme) -> container::Style {
    let c = colors(t);
    container::Style {
        background: Some(c.surface.into()),
        border: Border {
            color: c.border,
            width: 1.,
            radius: RADIUS_MD.into(),
        },
        ..Default::default()
    }
}
pub fn sidebar(t: &Theme) -> container::Style {
    container::Style {
        background: Some(colors(t).sidebar.into()),
        ..Default::default()
    }
}
pub fn rule(t: &Theme) -> container::Style {
    container::Style {
        background: Some(colors(t).border.into()),
        ..Default::default()
    }
}
pub fn chip(t: &Theme) -> container::Style {
    container::Style {
        background: Some(ACCENT.scale_alpha(0.12).into()),
        border: Border {
            radius: RADIUS_PILL.into(),
            ..Default::default()
        },
        text_color: Some(color(t, Ink::Accent)),
        ..Default::default()
    }
}
pub fn status_chip(active: bool) -> impl Fn(&Theme) -> container::Style {
    move |t| {
        if active {
            return chip(t);
        }
        container::Style {
            border: Border {
                color: colors(t).border,
                width: 1.,
                radius: RADIUS_PILL.into(),
            },
            text_color: Some(colors(t).muted),
            ..Default::default()
        }
    }
}
pub fn server_row(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |t, s| {
        let mut style = ghost(t, s);
        style.text_color = colors(t).text;
        style.border = Border {
            color: if active {
                ACCENT.scale_alpha(0.6)
            } else {
                Color::TRANSPARENT
            },
            width: 1.,
            radius: RADIUS_SM.into(),
        };
        if active {
            style.background = Some(ACCENT.scale_alpha(0.12).into());
        }
        style
    }
}
pub fn meter(role: Ink) -> impl Fn(&Theme) -> container::Style {
    move |t| container::Style {
        background: Some(color(t, role).into()),
        border: Border {
            radius: RADIUS_PILL.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
pub fn muted(t: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(colors(t).muted),
    }
}
pub fn faint(t: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(colors(t).faint),
    }
}
pub fn warning(t: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(color(t, Ink::Warning)),
    }
}
pub fn danger(t: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(color(t, Ink::Danger)),
    }
}
pub fn accent(t: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(color(t, Ink::Accent)),
    }
}
pub fn ghost(t: &Theme, status: button::Status) -> button::Style {
    let c = colors(t);
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(c.hover.into()),
        text_color: c.muted,
        border: Border {
            radius: RADIUS_PILL.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
/// A secondary action that must read as a button: ghost with a border.
pub fn outline(t: &Theme, status: button::Status) -> button::Style {
    let c = colors(t);
    let mut style = ghost(t, status);
    style.text_color = if matches!(status, button::Status::Disabled) {
        c.faint
    } else {
        c.text
    };
    style.border.width = 1.;
    style.border.color = c.border;
    style
}
pub fn column_heading(t: &Theme, status: button::Status) -> button::Style {
    let mut style = ghost(t, status);
    style.background = None;
    if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        style.text_color = colors(t).text;
    }
    style
}
pub fn link(t: &Theme, status: button::Status) -> button::Style {
    let mut style = ghost(t, status);
    style.text_color = color(t, Ink::Accent);
    style
}
pub fn picker(
    t: &Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    let mut style = iced::widget::pick_list::default(t, status);
    style.background = colors(t).bg.into();
    style.text_color = colors(t).text;
    style.border = Border {
        color: if t.extended_palette().is_dark {
            Color::from_rgb8(56, 62, 75)
        } else {
            Color::from_rgb8(200, 200, 200)
        },
        width: 1.,
        radius: RADIUS_SM.into(),
    };
    style
}
pub fn picker_menu(t: &Theme) -> iced::widget::overlay::menu::Style {
    let c = colors(t);
    iced::widget::overlay::menu::Style {
        background: c.surface.into(),
        border: Border {
            color: c.border,
            width: 1.,
            radius: RADIUS_SM.into(),
        },
        text_color: c.text,
        selected_text_color: c.text,
        selected_background: c.hover.into(),
        shadow: Shadow::default(),
    }
}
/// A navigation or list row that can be the current one.
pub fn selected(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |t, s| {
        let mut v = ghost(t, s);
        v.border.radius = RADIUS_SM.into();
        if active {
            v.background = Some(colors(t).hover.into());
            v.text_color = colors(t).text;
        }
        v
    }
}
/// One option inside a [`segments`] track, or a filter chip.
pub fn segment(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |t, s| {
        let mut v = selected(active)(t, s);
        v.border.radius = RADIUS_PILL.into();
        v
    }
}
pub fn primary(t: &Theme, status: button::Status) -> button::Style {
    let mut s = button::primary(t, status);
    s.border.radius = RADIUS_PILL.into();
    s.background = Some(Background::Color(
        if matches!(status, button::Status::Disabled) {
            ACCENT.scale_alpha(0.3)
        } else {
            ACCENT
        },
    ));
    s.text_color = if matches!(status, button::Status::Disabled) {
        Color::WHITE.scale_alpha(0.6)
    } else {
        Color::WHITE
    };
    s
}
pub fn power(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |t, status| button::Style {
        background: Some(
            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                if active {
                    Color::from_rgb8(35, 82, 210).into()
                } else if t.extended_palette().is_dark {
                    Color::from_rgb8(35, 68, 135).into()
                } else {
                    Color::from_rgb8(210, 224, 255).into()
                }
            } else if active {
                ACCENT.into()
            } else {
                colors(t).surface.into()
            },
        ),
        text_color: if active { Color::WHITE } else { colors(t).text },
        border: Border {
            color: if active {
                ACCENT
            } else {
                ACCENT.scale_alpha(0.35)
            },
            width: 1.,
            radius: RADIUS_PILL.into(),
        },
        shadow: if active {
            Shadow {
                color: ACCENT.scale_alpha(0.18),
                offset: iced::Vector::new(0., 6.),
                blur_radius: 20.,
            }
        } else {
            Shadow::default()
        },
        ..Default::default()
    }
}
pub fn input(t: &Theme, s: text_input::Status) -> text_input::Style {
    let mut v = text_input::default(t, s);
    v.background = colors(t).bg.into();
    v.border = Border {
        color: if matches!(s, text_input::Status::Focused { .. }) {
            ACCENT
        } else {
            colors(t).border
        },
        width: 1.,
        radius: RADIUS_SM.into(),
    };
    v
}
pub fn toggle(t: &Theme, status: toggler::Status) -> toggler::Style {
    let mut s = toggler::default(t, status);
    let on = match status {
        toggler::Status::Active { is_toggled }
        | toggler::Status::Hovered { is_toggled }
        | toggler::Status::Disabled { is_toggled } => is_toggled,
    };
    s.foreground = Color::WHITE.into();
    let dark = t.extended_palette().is_dark;
    let hovered = matches!(status, toggler::Status::Hovered { .. });
    s.background = match (on, hovered, dark) {
        (true, true, true) => Color::from_rgb8(80, 140, 255),
        (true, true, false) => Color::from_rgb8(30, 80, 210),
        (true, false, _) => ACCENT,
        (false, true, true) => Color::from_rgb8(95, 102, 118),
        (false, true, false) => Color::from_rgb8(150, 150, 150),
        (false, false, true) => Color::from_rgb8(56, 62, 75),
        (false, false, false) => Color::from_rgb8(205, 205, 205),
    }
    .into();
    s.background_border_width = 0.;
    s.foreground_border_width = 0.;
    s.padding_ratio = 0.12;
    s.border_radius = Some(RADIUS_PILL.into());
    s
}
pub fn scroll(t: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut s = scrollable::default(t, status);
    s.vertical_rail.background = None;
    s.vertical_rail.scroller.background = colors(t).faint.scale_alpha(0.5).into();
    s.vertical_rail.scroller.border.radius = RADIUS_PILL.into();
    s
}
pub fn segments(t: &Theme) -> container::Style {
    container::Style {
        background: Some(colors(t).sidebar.into()),
        border: Border {
            color: colors(t).border,
            width: 1.,
            radius: RADIUS_PILL.into(),
        },
        ..Default::default()
    }
}
