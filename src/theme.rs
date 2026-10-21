// SPDX-License-Identifier: GPL-3.0-or-later
// New styles implementing VesselWave's monochrome/blue design.
use iced::widget::{container, svg};
use iced::{Color, Theme};

pub const FONT: iced::Font = iced::Font::with_name("Geist");
pub const MONO: iced::Font = iced::Font::with_name("Geist Mono");
pub const ACCENT: Color = Color::from_rgb(47.0 / 255.0, 107.0 / 255.0, 1.0);
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum Ink {
    Text,
    Muted,
    Faint,
    Accent,
    Success,
    Warning,
    Danger,
}
pub fn ink(role: Ink) -> impl Fn(&Theme, svg::Status) -> svg::Style {
    move |t, _| svg::Style {
        color: Some(match role {
            Ink::Accent => ACCENT,
            Ink::Success => Color::from_rgb(0.2, 0.7, 0.4),
            Ink::Warning => Color::from_rgb(0.85, 0.6, 0.1),
            Ink::Danger => Color::from_rgb(0.9, 0.25, 0.3),
            Ink::Text => t.palette().text,
            Ink::Muted => t.palette().text.scale_alpha(0.65),
            Ink::Faint => t.palette().text.scale_alpha(0.4),
        }),
    }
}
pub fn palette(dark: bool) -> Theme {
    Theme::custom(
        "Wave",
        iced::theme::Palette {
            background: if dark {
                Color::from_rgb8(14, 14, 16)
            } else {
                Color::from_rgb8(250, 250, 250)
            },
            text: if dark {
                Color::from_rgb8(240, 240, 243)
            } else {
                Color::from_rgb8(24, 24, 27)
            },
            primary: ACCENT,
            success: Color::from_rgb(0.2, 0.7, 0.4),
            warning: Color::from_rgb(0.85, 0.6, 0.1),
            danger: Color::from_rgb(0.9, 0.25, 0.3),
        },
    )
}
pub fn panel(t: &Theme) -> container::Style {
    container::Style {
        background: Some(t.extended_palette().background.weak.color.into()),
        border: iced::Border {
            color: t.extended_palette().background.strong.color,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}
