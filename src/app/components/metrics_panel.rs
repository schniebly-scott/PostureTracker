use std::time::Duration;

use iced::border::Border;
use iced::widget::{button, column, container, progress_bar, row, stack, text, Space};
use iced::{Alignment, Background, Color, Element, Font, Length};

use crate::app::components::slide::slide;
use crate::app::components::ui;
use crate::app::theme::{DARK_BLUE, LIGHT_BLUE, LINE, MID_BLUE, OWHITE, WARNING_RED};
use crate::app::{App, MetricsCategory, Message, SlideDirection};

fn fmt_duration(d: Option<Duration>) -> String {
    let Some(d) = d else {
        return "--".to_string();
    };
    let secs = d.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn fmt_duration_long(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        format!("{hours}h {minutes:02}m")
    }
}

fn bold() -> Font {
    ui::bold()
}

fn lighten(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r + amount).min(1.0),
        g: (color.g + amount).min(1.0),
        b: (color.b + amount).min(1.0),
        a: color.a,
    }
}

// Raised tile surface (matches the new design's ELEV token).
const CARD_BG: Color = Color {
    r: 0x23 as f32 / 255.0,
    g: 0x28 as f32 / 255.0,
    b: 0x2F as f32 / 255.0,
    a: 1.0,
};

// Hero card — a touch lighter than the tiles so it still leads (HOVER token).
const PRIMARY_BG: Color = Color {
    r: 0x2A as f32 / 255.0,
    g: 0x30 as f32 / 255.0,
    b: 0x38 as f32 / 255.0,
    a: 1.0,
};

// Reset popup surface (PANEL token).
const POPUP_BG: Color = Color {
    r: 0x1B as f32 / 255.0,
    g: 0x1F as f32 / 255.0,
    b: 0x25 as f32 / 255.0,
    a: 1.0,
};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = view_header(app.metrics_category);
    let body = view_body(app);
    let footer = view_footer();

    let panel = container(
        column![header, body, footer]
            .spacing(10)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .style(ui::panel_style)
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .clip(true);

    if app.metrics_reset_open {
        // Anchor the confirm popup over the full panel. The panel is the stack's
        // first (sizing) layer, so the popup gets the whole panel area to lay out
        // in rather than being clipped to the short footer row.
        let overlay = container(reset_popup(app.metrics_category))
            .align_x(Alignment::End)
            .align_y(Alignment::End)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([56, 16]);
        stack![panel, overlay].into()
    } else {
        panel.into()
    }
}

fn view_header(category: MetricsCategory) -> Element<'static, Message> {
    let arrow_style = |_theme: &iced::Theme, status: button::Status| {
        let alpha = match status {
            button::Status::Hovered | button::Status::Pressed => 1.0,
            _ => 0.7,
        };
        button::Style {
            background: None,
            text_color: Color { a: alpha, ..OWHITE },
            border: Border::default().rounded(4),
            ..button::Style::default()
        }
    };

    let left = button(ui::icon(ui::glyph::CHEVRON_LEFT, 28))
        .on_press(Message::MetricsCategoryCycled(SlideDirection::Left))
        .style(arrow_style)
        .padding([0, 10]);

    let right = button(ui::icon(ui::glyph::CHEVRON_RIGHT, 28))
        .on_press(Message::MetricsCategoryCycled(SlideDirection::Right))
        .style(arrow_style)
        .padding([0, 10]);

    let title = text(category.label()).size(22).font(bold());

    row![
        left,
        Space::new().width(Length::Fill),
        title,
        Space::new().width(Length::Fill),
        right,
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

fn view_body(app: &App) -> Element<'_, Message> {
    let current = view_category(app, app.metrics_category);
    let (previous, direction, progress) = match app.metrics_transition {
        Some(t) => (
            Some(view_category(app, t.from)),
            t.direction,
            t.progress(),
        ),
        None => (None, SlideDirection::Right, 1.0),
    };

    container(slide(current, previous, progress, direction))
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .into()
}

fn view_category(app: &App, category: MetricsCategory) -> Element<'_, Message> {
    match category {
        MetricsCategory::Daily => view_daily(app),
        MetricsCategory::Session => view_session(app),
        MetricsCategory::AllTime => view_all_time(app),
        MetricsCategory::QuickView => view_quick(app),
    }
}

fn view_daily(app: &App) -> Element<'_, Message> {
    let m = &app.metrics;
    let primary = primary_card(
        ui::glyph::CLOCK,
        "TOTAL TIME TODAY",
        fmt_duration_long(m.tracked_duration_today()),
        LIGHT_BLUE,
    );

    let row_a = row![
        secondary_card(ui::glyph::TRIANGLE, "Breaks", m.breaks_today().to_string(), false),
        secondary_card(
            ui::glyph::CROSS,
            "Bad time",
            fmt_duration_long(m.bad_posture_duration_today()),
            true,
        ),
    ]
    .spacing(8);

    let streak = m.good_posture_streak();
    let row_b = row![
        secondary_card(
            if streak.is_some() { ui::glyph::DISC } else { ui::glyph::CIRCLE },
            "Streak",
            fmt_duration(streak),
            false,
        ),
        secondary_card(
            ui::glyph::HALF_DISC,
            "Since break",
            fmt_duration(m.time_since_last_break()),
            false,
        ),
    ]
    .spacing(8);

    column![primary, row_a, row_b]
        .spacing(8)
        .width(Length::Fill)
        .into()
}

fn view_session(app: &App) -> Element<'_, Message> {
    let m = &app.metrics;
    let session_active = m.is_session_active();

    let primary_value = if session_active {
        fmt_duration_long(m.tracked_duration_session())
    } else {
        "--".to_string()
    };

    let primary = primary_card(
        ui::glyph::CLOCK,
        "SESSION LENGTH",
        primary_value,
        if session_active { LIGHT_BLUE } else { OWHITE },
    );

    let row_a = row![
        secondary_card(ui::glyph::TRIANGLE, "Breaks", m.breaks_session().to_string(), false),
        secondary_card(
            ui::glyph::CROSS,
            "Bad time",
            fmt_duration_long(m.bad_posture_duration_session()),
            true,
        ),
    ]
    .spacing(8);

    let quality = m.posture_quality_session();
    let quality_card = container(
        row![
            row![
                ui::icon(ui::glyph::BARS, 13),
                text("Posture Quality").size(12).color(Color { a: 0.75, ..OWHITE }),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            Space::new().width(Length::Fill),
            progress_bar(0.0..=1.0, quality).length(100).girth(8),
            text(format!("{:.0}%", quality * 100.0))
                .size(16)
                .font(bold())
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding([10, 12])
    .width(Length::Fill)
    .clip(true)
    .style(|_| container::Style {
        background: Some(Background::Color(CARD_BG)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    });

    column![primary, row_a, quality_card]
        .spacing(8)
        .width(Length::Fill)
        .into()
}

fn view_all_time(app: &App) -> Element<'_, Message> {
    let m = &app.metrics;

    let primary = primary_card(
        ui::glyph::CLOCK,
        "LIFETIME TRACKED",
        fmt_duration_long(m.all_time_tracked_duration()),
        LIGHT_BLUE,
    );

    let row_a = row![
        secondary_card(
            ui::glyph::TRIANGLE,
            "Lifetime breaks",
            m.all_time_breaks().to_string(),
            false,
        ),
        secondary_card(
            ui::glyph::CROSS,
            "Bad time",
            fmt_duration_long(m.all_time_bad_posture_duration()),
            true,
        ),
    ]
    .spacing(8);

    let days = m.all_time_days_tracked();
    let avg_breaks = if days > 0 {
        format!("{:.1}", m.all_time_breaks() as f32 / days as f32)
    } else {
        "--".to_string()
    };

    let row_b = row![
        secondary_card(ui::glyph::GRID, "Days tracked", days.to_string(), false),
        secondary_card(ui::glyph::TREND_UP, "Avg breaks/day", avg_breaks, false),
    ]
    .spacing(8);

    column![primary, row_a, row_b]
        .spacing(8)
        .width(Length::Fill)
        .into()
}

fn view_quick(app: &App) -> Element<'_, Message> {
    let m = &app.metrics;

    let streak = m.good_posture_streak();
    let streak_ok = streak.is_some();
    let streak_card = quick_card(
        if streak_ok { ui::glyph::DISC } else { ui::glyph::CIRCLE },
        "STREAK",
        fmt_duration(streak),
        None,
        if streak_ok { LIGHT_BLUE } else { WARNING_RED },
    );

    let breaks = m.breaks_today();
    let breaks_card = quick_card(
        ui::glyph::TRIANGLE,
        "BREAKS TODAY",
        breaks.to_string(),
        None,
        if breaks == 0 { LIGHT_BLUE } else { WARNING_RED },
    );

    let quality = m.posture_quality_today();
    let quality_color = if quality >= 0.8 {
        LIGHT_BLUE
    } else if quality >= 0.5 {
        OWHITE
    } else {
        WARNING_RED
    };
    let quality_card = quick_card(
        ui::glyph::BARS,
        "POSTURE QUALITY",
        format!("{:.0}%", quality * 100.0),
        Some(quality),
        quality_color,
    );

    column![streak_card, breaks_card, quality_card]
        .spacing(10)
        .width(Length::Fill)
        .into()
}

fn primary_card<'a>(
    glyph: &'a str,
    label: &'a str,
    value: String,
    accent: Color,
) -> Element<'a, Message> {
    let label_block = row![
        ui::icon(glyph, 18),
        text(label).size(12).font(bold()).color(Color {
            a: 0.75,
            ..OWHITE
        }),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let value_text = text(value)
        .size(23)
        .font(bold())
        .wrapping(iced::widget::text::Wrapping::None);

    let card = container(
        row![
            label_block,
            Space::new().width(Length::Fill),
            value_text,
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding([12, 14])
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(PRIMARY_BG)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    });

    let stripe = container(Space::new().width(4).height(Length::Fill))
        .style(move |_| container::Style {
            background: Some(Background::Color(accent)),
            border: Border::default().rounded(2),
            ..Default::default()
        });

    row![stripe, card]
        .spacing(6)
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .into()
}

fn secondary_card<'a>(
    glyph: &'a str,
    label: &'a str,
    value: String,
    is_bad: bool,
) -> Element<'a, Message> {
    let value_color = if is_bad {
        lighten(WARNING_RED, 0.05)
    } else {
        OWHITE
    };

    container(
        row![
            row![
                ui::icon(glyph, 13).color(OWHITE),
                text(label).size(12).color(OWHITE),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            Space::new().width(Length::Fill),
            text(value)
                .size(17)
                .font(bold())
                .color(value_color)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding([10, 12])
    .width(Length::Fill)
    .clip(true)
    .style(|_| container::Style {
        background: Some(Background::Color(CARD_BG)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn quick_card<'a>(
    glyph: &'a str,
    label: &'a str,
    value: String,
    progress: Option<f32>,
    accent: Color,
) -> Element<'a, Message> {
    let label_block = row![
        ui::icon(glyph, 18).color(accent),
        text(label).size(12).font(bold()).color(Color {
            a: 0.75,
            ..OWHITE
        }),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let value_text = text(value)
        .size(24)
        .font(bold())
        .wrapping(iced::widget::text::Wrapping::None);

    let inner: Element<'_, Message> = if let Some(p) = progress {
        row![
            label_block,
            Space::new().width(Length::Fill),
            progress_bar(0.0..=1.0, p).length(120).girth(10),
            value_text,
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
    } else {
        row![
            label_block,
            Space::new().width(Length::Fill),
            value_text,
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
    };

    let card = container(inner)
        .padding([12, 14])
        .width(Length::Fill)
        .clip(true)
        .style(|_| container::Style {
            background: Some(Background::Color(PRIMARY_BG)),
            border: Border {
                color: LINE,
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        });

    let stripe = container(Space::new().width(4).height(Length::Fill))
        .style(move |_| container::Style {
            background: Some(Background::Color(accent)),
            border: Border::default().rounded(2),
            ..Default::default()
        });

    row![stripe, card]
        .spacing(6)
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .into()
}

fn view_footer() -> Element<'static, Message> {
    let reset_btn = button(
        row![
            ui::icon(ui::glyph::REFRESH, 18),
            text("Reset").size(13).font(bold()),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(Message::MetricsResetMenuToggled)
        .padding([4, 10])
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Color { a: 0.6, ..DARK_BLUE }
                }
                _ => Color { a: 0.3, ..DARK_BLUE },
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: OWHITE,
                border: Border::default().rounded(6),
                ..button::Style::default()
            }
        });

    row![Space::new().width(Length::Fill), reset_btn]
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .into()
}

fn reset_popup(category: MetricsCategory) -> Element<'static, Message> {
    let cancel = button(text("Cancel").size(13))
        .on_press(Message::MetricsResetMenuToggled)
        .padding([4, 10])
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => MID_BLUE,
                _ => DARK_BLUE,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: OWHITE,
                border: Border::default().rounded(4),
                ..button::Style::default()
            }
        });

    let confirm = button(text("Reset").size(13).font(bold()))
        .on_press(Message::MetricsResetConfirmed)
        .padding([4, 10])
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => lighten(WARNING_RED, 0.05),
                _ => WARNING_RED,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: OWHITE,
                border: Border::default().rounded(4),
                ..button::Style::default()
            }
        });

    container(
        column![
            text(format!("Reset {}?", category.label()))
                .size(13)
                .font(bold()),
            row![cancel, confirm].spacing(8),
        ]
        .spacing(8),
    )
    .padding(10)
    .style(|_| container::Style {
        background: Some(Background::Color(POPUP_BG)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    })
    .into()
}
