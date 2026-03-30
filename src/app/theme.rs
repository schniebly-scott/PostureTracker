use iced::Color;
use iced::Theme;
use iced::theme::Palette;

pub fn custom_theme() -> Theme {
    Theme::custom(
        "custom",
        Palette {
            background: DARK_BLUE,
            text: OWHITE,
            primary: MID_BLUE,
            success: LIGHT_BLUE,
            danger: OBROWN,
            warning: WARNING_RED,
        },
    )
}

pub const OWHITE: Color = Color::from_rgb(
    0xE3 as f32 / 255.0,
    0xE8 as f32 / 255.0,
    0xE3 as f32 / 255.0,
);

pub const OBROWN: Color = Color::from_rgb(
    0x66 as f32 / 255.0,
    0x63 as f32 / 255.0,
    0x5A as f32 / 255.0,
);

pub const LIGHT_BLUE: Color = Color::from_rgb(
    0x7E as f32 / 255.0,
    0x99 as f32 / 255.0,
    0x9F as f32 / 255.0,
);

pub const MID_BLUE: Color = Color::from_rgb(
    0x4E as f32 / 255.0,
    0x53 as f32 / 255.0,
    0x5B as f32 / 255.0,
);

pub const DARK_BLUE: Color = Color::from_rgb(
    0x2E as f32 / 255.0,
    0x33 as f32 / 255.0,
    0x3E as f32 / 255.0,
);

pub const WARNING_RED: Color = Color::from_rgb(
    0xC7 as f32 / 255.0,
    0x5C as f32 / 255.0,
    0x5C as f32 / 255.0,
);
