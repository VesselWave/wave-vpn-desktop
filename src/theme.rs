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
            bg: Color::from_rgb8(10, 10, 10),
            sidebar: Color::from_rgb8(17, 17, 17),
            surface: Color::from_rgb8(20, 20, 20),
            border: Color::from_rgb8(38, 38, 38),
            muted: Color::from_rgb8(161, 161, 161),
            faint: Color::from_rgb8(107, 107, 107),
            text: Color::from_rgb8(237, 237, 237),
            hover: Color::from_rgb8(36, 36, 36),
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
        Ink::Accent => {
            if t.extended_palette().is_dark {
                Color::from_rgb8(110, 155, 255)
            } else {
                ACCENT
            }
        }
        Ink::OnAccent => Color::WHITE,
        Ink::Success => Color::from_rgb8(63, 207, 142),
        Ink::Warning => Color::from_rgb8(245, 165, 36),
        Ink::Danger => Color::from_rgb8(255, 92, 92),
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
            radius: 12.into(),
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
            radius: 999.into(),
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
                radius: 999.into(),
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
            radius: 8.into(),
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
            radius: 99.into(),
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
            radius: 8.into(),
            ..Default::default()
        },
        ..Default::default()
    }
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
        color: colors(t).border,
        width: 1.,
        radius: 8.into(),
    };
    style
}
pub fn selected(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |t, s| {
        let mut v = ghost(t, s);
        if active {
            v.background = Some(colors(t).hover.into());
            v.text_color = colors(t).text;
        }
        v
    }
}
pub fn primary(t: &Theme, status: button::Status) -> button::Style {
    let mut s = button::primary(t, status);
    s.border.radius = 8.into();
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
        background: Some(if active || matches!(status, button::Status::Hovered) {
            ACCENT.into()
        } else {
            colors(t).surface.into()
        }),
        text_color: if active {
            Color::WHITE
        } else {
            color(t, Ink::Accent)
        },
        border: Border {
            color: if active {
                ACCENT
            } else {
                ACCENT.scale_alpha(0.35)
            },
            width: 1.,
            radius: 999.into(),
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
        radius: 8.into(),
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
    s.background = if on {
        ACCENT
    } else {
        Color::from_rgb8(58, 58, 58)
    }
    .into();
    s.background_border_width = 0.;
    s.foreground_border_width = 0.;
    s.padding_ratio = 0.12;
    s.border_radius = Some(999.into());
    s
}
pub fn scroll(t: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut s = scrollable::default(t, status);
    s.vertical_rail.background = None;
    s.vertical_rail.scroller.background = colors(t).faint.scale_alpha(0.5).into();
    s.vertical_rail.scroller.border.radius = 999.into();
    s
}
pub fn segments(t: &Theme) -> container::Style {
    container::Style {
        background: Some(colors(t).sidebar.into()),
        border: Border {
            color: colors(t).border,
            width: 1.,
            radius: 8.into(),
        },
        ..Default::default()
    }
}
