//! Line icons as inline SVG, tinted per theme via [`theme::ink`].
//! Paths adapted from Lucide (https://lucide.dev); see resources/LUCIDE-LICENSE.txt.

use iced::widget::svg;
use iced::Element;

use crate::theme::{self, Ink};

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)] // Retained authored icon set; not every icon is used by the new shell yet.
pub enum Icon {
    Gauge,
    Globe,
    Logs,
    Sliders,
    Power,
    Lock,
    LockOpen,
    Refresh,
    ArrowDown,
    ArrowUp,
    ChevronUp,
    ChevronDown,
    ChevronRight,
    Alert,
    Wrench,
    Star,
    StarFilled,
    Zap,
}

/// Lucide's star outline, shared by the outline and filled variants.
macro_rules! star_path {
    () => {
        "M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"
    };
}

impl Icon {
    fn body(self) -> &'static str {
        match self {
            Icon::Star => concat!(r#"<path d=""#, star_path!(), r#""/>"#),
            Icon::StarFilled => concat!(r#"<path d=""#, star_path!(), r#"" fill="black"/>"#),
            Icon::Zap => {
                r#"<path d="M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z"/>"#
            }
            Icon::Gauge => r#"<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>"#,
            Icon::Globe => {
                r#"<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/>"#
            }
            Icon::Logs => {
                r#"<path d="M13 12h8"/><path d="M13 18h8"/><path d="M13 6h8"/><path d="M3 12h1"/><path d="M3 18h1"/><path d="M3 6h1"/><path d="M8 12h1"/><path d="M8 18h1"/><path d="M8 6h1"/>"#
            }
            Icon::Sliders => {
                r#"<path d="M10 5H3"/><path d="M12 19H3"/><path d="M14 3v4"/><path d="M16 17v4"/><path d="M21 12h-9"/><path d="M21 19h-5"/><path d="M21 5h-7"/><path d="M8 10v4"/><path d="M8 12H3"/>"#
            }
            Icon::Power => r#"<path d="M12 2v10"/><path d="M18.4 6.6a9 9 0 1 1-12.77.04"/>"#,
            Icon::Lock => {
                r#"<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>"#
            }
            Icon::LockOpen => {
                r#"<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 9.9-1"/>"#
            }
            Icon::Refresh => {
                r#"<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>"#
            }
            Icon::ArrowDown => r#"<path d="M12 5v14"/><path d="m19 12-7 7-7-7"/>"#,
            Icon::ArrowUp => r#"<path d="m5 12 7-7 7 7"/><path d="M12 19V5"/>"#,
            Icon::ChevronUp => r#"<path d="m18 15-6-6-6 6"/>"#,
            Icon::ChevronDown => r#"<path d="m6 9 6 6 6-6"/>"#,
            Icon::ChevronRight => r#"<path d="m9 18 6-6-6-6"/>"#,
            Icon::Alert => {
                r#"<circle cx="12" cy="12" r="10"/><path d="M12 8v4"/><path d="M12 16h.01"/>"#
            }
            Icon::Wrench => {
                r#"<path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>"#
            }
        }
    }

    fn handle(self) -> svg::Handle {
        let doc = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#,
            self.body()
        );
        svg::Handle::from_memory(doc.into_bytes())
    }
}

/// An icon at `size` px, tinted with `role`.
pub fn icon<'a, M: 'a>(which: Icon, size: f32, role: Ink) -> Element<'a, M> {
    svg(which.handle())
        .width(size)
        .height(size)
        .style(theme::ink(role))
        .into()
}
