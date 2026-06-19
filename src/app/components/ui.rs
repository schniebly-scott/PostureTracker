//! Direction A — "Refined Slate" style kit.
//!
//! Shared building blocks so every dashboard panel renders with one button
//! system, one set of surfaces, and one type scale (uppercase micro-labels,
//! monospace numbers).

use iced::border::Border;
use iced::font::Weight;
use iced::widget::{button, container, text};
use iced::{Background, Color, Element, Font};

use crate::app::Message;
use crate::app::theme::{
    ELEV, GREEN, HOVER, LINE, LINE_STRONG, ON_GREEN, PANEL, RED, SEL, T1, T2, T3,
};

// ---- Fonts ----
//
// Every face the app renders with is bundled and loaded at startup (see
// `app::run`), so glyphs never depend on per-OS system-font fallback. That
// fallback is what made the old geometric/dingbat "icons" render at different
// widths and baselines on Linux/Windows/macOS. Text is Inter, numbers are
// JetBrains Mono, and icons are a tiny purpose-built [`ICON_FONT`] — see
// [`icon`] and [`glyph`].

/// Inter (regular weight) — the app's default text font, set via
/// `default_font` in `app::run`. Loaded from [`INTER_REGULAR`].
pub const INTER: Font = Font::with_name("Inter");

/// JetBrains Mono — bundled monospace used for every number so values read
/// instantly. Loaded from [`JETBRAINS_MONO`].
pub const MONO: Font = Font::with_name("JetBrains Mono");

/// Bundled icon font holding only the handful of UI glyphs the app uses, so
/// each renders identically on every OS. Reach it through [`icon`], not by
/// naming a raw glyph in a default-font `text`.
pub const ICONS: Font = Font::with_name("Posture Icons");

/// Raw bytes for the bundled fonts. `app::run` hands these to iced's font
/// loader; keep that list in sync with these constants.
pub const INTER_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
pub const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");
pub const INTER_BOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-Bold.ttf");
pub const JETBRAINS_MONO: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
pub const ICON_FONT: &[u8] = include_bytes!("../../../assets/fonts/PostureIcons.ttf");

pub fn semibold() -> Font {
    Font {
        weight: Weight::Semibold,
        ..INTER
    }
}

pub fn bold() -> Font {
    Font {
        weight: Weight::Bold,
        ..INTER
    }
}

/// Bundled JetBrains Mono, used for every number so values read instantly.
pub fn mono() -> Font {
    MONO
}

// ---- Icons ----
//
// The canonical name for every UI glyph. Routing all icon usage through these
// constants and [`icon`] keeps raw `\u{...}` strings out of the panels — a raw
// glyph in a default-font `text` would resolve via system fallback and reopen
// the cross-platform inconsistency this module exists to prevent.
pub mod glyph {
    pub const STOP: &str = "\u{25A0}"; // ■  stop tracking / stop test
    pub const PLAY: &str = "\u{25B6}"; // ▶  test / start session
    pub const TARGET: &str = "\u{25CE}"; // ◎  calibrate
    pub const GEAR: &str = "\u{2699}"; // ⚙  settings
    pub const CAMERA: &str = "\u{25A3}"; // ▣  idle camera
    pub const CLOCK: &str = "\u{25F7}"; // ◷  elapsed / total time
    pub const TRIANGLE: &str = "\u{25B3}"; // △  breaks
    pub const CROSS: &str = "\u{2715}"; // ✕  bad time
    pub const DISC: &str = "\u{25CF}"; // ●  active streak
    pub const CIRCLE: &str = "\u{25CB}"; // ○  no streak
    pub const HALF_DISC: &str = "\u{25D0}"; // ◐  since break
    pub const BARS: &str = "\u{2261}"; // ≡  posture quality
    pub const GRID: &str = "\u{25A4}"; // ▤  days tracked
    pub const TREND_UP: &str = "\u{2197}"; // ↗  average per day
    pub const REFRESH: &str = "\u{21BB}"; // ↻  reset
    pub const CHEVRON_LEFT: &str = "\u{2039}"; // ‹  previous category
    pub const CHEVRON_RIGHT: &str = "\u{203A}"; // ›  next category
}

/// An icon glyph rendered in the bundled [`ICONS`] font at `size`. Set the
/// color on the returned text as usual. Always build icons through this helper
/// (or [`icon_button`]) rather than putting a glyph in a plain `text`.
pub fn icon(glyph: &str, size: u16) -> text::Text<'_> {
    text(glyph)
        .size(size as f32)
        .font(ICONS)
        .wrapping(text::Wrapping::None)
}

// ---- Color helpers ----

pub fn with_alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

/// Linear blend `amount` of `b` into `a` (0 = all `a`, 1 = all `b`).
pub fn mix(a: Color, b: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

// ---- Type helpers ----

/// Uppercase, letter-spaced tertiary label that leads a section.
pub fn micro_label(label: &str) -> text::Text<'_> {
    text(label.to_uppercase())
        .size(12)
        .font(semibold())
        .color(T3)
}

/// Monospace numeric value.
pub fn value(s: impl text::IntoFragment<'static>, size: u16, color: Color) -> text::Text<'static> {
    text(s)
        .size(size as f32)
        .font(mono())
        .color(color)
        .wrapping(text::Wrapping::None)
}

// ---- Surfaces ----

/// A first-level panel (rounded card with hairline border).
pub fn panel_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(PANEL)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 14.0.into(),
        },
        ..Default::default()
    }
}

/// A panel that picks up a red-tinted border when posture is bad ("clear but
/// firm" — scoped, never a full wash).
pub fn panel_alert_style(alert: bool) -> impl Fn(&iced::Theme) -> container::Style {
    move |_theme| {
        let border_color = if alert {
            mix(LINE, RED, 0.45)
        } else {
            LINE
        };
        container::Style {
            background: Some(Background::Color(PANEL)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 14.0.into(),
            },
            ..Default::default()
        }
    }
}

/// A raised tile / chip surface.
pub fn tile_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(ELEV)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

// ---- Button system ----

/// Solid green call-to-action.
pub fn primary_button(label: Element<'_, Message>) -> button::Button<'_, Message> {
    button(label)
        .padding([12, 12])
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => mix(GREEN, Color::WHITE, 0.12),
                _ => GREEN,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: ON_GREEN,
                border: Border::default().rounded(10),
                ..button::Style::default()
            }
        })
}

/// Tinted destructive action.
pub fn danger_button(label: Element<'_, Message>) -> button::Button<'_, Message> {
    button(label)
        .padding([12, 12])
        .style(|_theme, status| {
            let base = mix(PANEL, RED, 0.16);
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => mix(PANEL, RED, 0.24),
                _ => base,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: RED,
                border: Border {
                    color: with_alpha(RED, 0.40),
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..button::Style::default()
            }
        })
}

/// Neutral filled button.
pub fn secondary_button(label: Element<'_, Message>) -> button::Button<'_, Message> {
    button(label)
        .padding([12, 12])
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => HOVER,
                _ => ELEV,
            };
            let border_color = match status {
                button::Status::Hovered | button::Status::Pressed => LINE_STRONG,
                _ => LINE,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: T1,
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..button::Style::default()
            }
        })
}

/// Quiet outlined button (used for Re-calibrate during a session).
pub fn ghost_button(label: Element<'_, Message>) -> button::Button<'_, Message> {
    button(label)
        .padding([12, 15])
        .style(|_theme, status| {
            let (bg, text_color) = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    (Some(Background::Color(HOVER)), T1)
                }
                _ => (None, T2),
            };
            button::Style {
                background: bg,
                text_color,
                border: Border {
                    color: LINE,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..button::Style::default()
            }
        })
}

/// Inert button used for in-progress states (no `on_press`).
pub fn disabled_button(label: Element<'_, Message>) -> button::Button<'_, Message> {
    button(label).padding([12, 15]).style(|_theme, _status| {
        button::Style {
            background: Some(Background::Color(with_alpha(ELEV, 0.6))),
            text_color: T3,
            border: Border {
                color: LINE,
                width: 1.0,
                radius: 10.0.into(),
            },
            ..button::Style::default()
        }
    })
}

/// One segment of a segmented pill control. `selected` paints the active fill;
/// otherwise the segment is transparent and only brightens its text on hover.
pub fn seg_button(label: &str, selected: bool, msg: Message) -> button::Button<'_, Message> {
    button(
        text(label)
            .size(13)
            .font(semibold())
            .wrapping(text::Wrapping::None),
    )
    .padding([7, 10])
    .on_press(msg)
    .style(move |_theme, status| {
        let (bg, tc) = if selected {
            (Some(Background::Color(SEL)), T1)
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => (None, T1),
                _ => (None, T3),
            }
        };
        button::Style {
            background: bg,
            text_color: tc,
            border: Border::default().rounded(7),
            ..button::Style::default()
        }
    })
}

/// Quiet square icon button for toolbars / titlebar.
pub fn icon_button(glyph: &'static str, size: u16) -> button::Button<'static, Message> {
    button(icon(glyph, size).color(T3))
        .padding([6, 8])
        .style(|_theme, status| {
            let (bg, text_color) = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    (Some(Background::Color(HOVER)), T1)
                }
                _ => (None, T3),
            };
            button::Style {
                background: bg,
                text_color,
                border: Border::default().rounded(9),
                ..button::Style::default()
            }
        })
}
