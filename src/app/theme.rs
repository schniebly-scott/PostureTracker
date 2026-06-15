use iced::Color;
use iced::Theme;
use iced::theme::Palette;

/// Build a color from a 0xRRGGBB literal.
const fn rgb(hex: u32) -> Color {
    Color::from_rgb(
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    )
}

/// A translucent white, used for the design's hairline borders.
pub const fn white_a(a: f32) -> Color {
    Color { r: 1.0, g: 1.0, b: 1.0, a }
}

pub fn custom_theme() -> Theme {
    Theme::custom(
        "PostureTracker — Refined Slate",
        Palette {
            background: BG,
            text: T1,
            primary: GREEN,
            success: GREEN,
            danger: RED,
            warning: AMBER,
        },
    )
}

// ============================================================
// Direction A — "Refined Slate" design tokens
// ============================================================

/// Window backdrop behind the panels.
pub const BG: Color = rgb(0x15181D);
/// Default panel surface.
pub const PANEL: Color = rgb(0x1B1F25);
/// Raised surface — tiles, tracks, chips.
pub const ELEV: Color = rgb(0x23282F);
/// Hover state for interactive surfaces.
pub const HOVER: Color = rgb(0x2A3038);
/// Selected segment background.
pub const SEL: Color = rgb(0x323943);

/// Hairline border.
pub const LINE: Color = white_a(0.07);
/// Stronger hairline, used on hover/emphasis.
pub const LINE_STRONG: Color = white_a(0.14);

/// Primary text.
pub const T1: Color = rgb(0xE8EBEF);
/// Secondary text.
pub const T2: Color = rgb(0xA3ACB7);
/// Tertiary / micro-label text.
pub const T3: Color = rgb(0x6C7682);

/// Green accent (good posture, primary actions).
pub const GREEN: Color = rgb(0x3DDC97);
/// Ink used on top of the green accent.
pub const ON_GREEN: Color = rgb(0x062014);

/// Red alert (bad posture, danger).
pub const RED: Color = rgb(0xF26D78);

/// Amber warning.
pub const AMBER: Color = rgb(0xE8B15A);

/// Near-black matte behind the camera feed, like a video player.
pub const VIDEO_BG: Color = rgb(0x0C0E11);
/// Translucent ink for chips/scrims floating over the feed.
pub const SCRIM: Color = Color {
    r: 0x0A as f32 / 255.0,
    g: 0x0C as f32 / 255.0,
    b: 0x0F as f32 / 255.0,
    a: 0.72,
};
