use std::time::Duration;

use iced::border::Border;
use iced::widget::{
    button, column, container, progress_bar, responsive, row, stack, text, Space,
};
use iced::{Alignment, Background, Color, Element, Length, Padding};

use crate::app::components::slide::slide;
use crate::app::components::ui;
use crate::app::components::ui::Scale;
use crate::app::theme::{ELEV, GREEN, HOVER, LINE, PANEL, RED, T1};
use crate::app::{App, MetricsCategory, Message, SlideDirection};

const FOOTER_HEIGHT: f32 = 30.0;
const PANEL_SPACING: f32 = 10.0;

/// Natural (scale-1) heights of each category's card stack, used to shrink the
/// cards to fit the body height they actually get (see [`view_body`]). These
/// are the tuning knobs if a card's bottom edge ever clips: measure the stack
/// at scale 1 and round up.
const BODY_NATURAL_STANDARD: f32 = 156.0;
const BODY_NATURAL_QUICK: f32 = 188.0;

fn natural_body_height(category: MetricsCategory) -> f32 {
    match category {
        MetricsCategory::QuickView => BODY_NATURAL_QUICK,
        _ => BODY_NATURAL_STANDARD,
    }
}

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

/// Untracked cards show a neutral "--" rather than a misleading `0`/`0s`, so the
/// whole dashboard uses one empty-state convention (matching `fmt_duration(None)`).
fn dash_if_untracked(tracked: Duration, value: String) -> String {
    if tracked.is_zero() {
        "--".to_string()
    } else {
        value
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

pub fn view(app: &App, scale: Scale) -> Element<'_, Message> {
    let header = view_header(app.metrics_category, scale);
    let body = view_body(app, scale);
    let footer = view_footer(scale);

    let panel = container(
        column![header, body, footer]
            .spacing(scale.px(PANEL_SPACING))
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .style(ui::panel_style)
    .padding(scale.pad_all(16.0))
    .width(Length::Fill)
    .height(Length::Fill)
    .clip(true);

    if app.metrics_reset_open {
        // Float the confirm popup over the full panel (the panel is the stack's
        // first/sizing layer, so the popup gets the whole panel area to lay out
        // in rather than being clipped to the short footer row). Anchor it to
        // the bottom-right and lift it clear of the footer by the footer's own
        // height plus the column spacing — no magic offset to keep in sync.
        let overlay = container(reset_popup(app.metrics_category, scale))
            .align_x(Alignment::End)
            .align_y(Alignment::End)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(
                Padding::ZERO
                    .right(scale.px(16.0))
                    .bottom(scale.px(FOOTER_HEIGHT + PANEL_SPACING)),
            );
        stack![panel, overlay].into()
    } else {
        panel.into()
    }
}

fn view_header(category: MetricsCategory, scale: Scale) -> Element<'static, Message> {
    let arrow_style = |_theme: &iced::Theme, status: button::Status| {
        let alpha = match status {
            button::Status::Hovered | button::Status::Pressed => 1.0,
            _ => 0.7,
        };
        button::Style {
            background: None,
            text_color: Color { a: alpha, ..T1 },
            border: Border::default().rounded(4),
            ..button::Style::default()
        }
    };

    let left = button(ui::icon(ui::glyph::CHEVRON_LEFT, scale.px(28.0)))
        .on_press(Message::MetricsCategoryCycled(SlideDirection::Left))
        .style(arrow_style)
        .padding(scale.pad(0.0, 10.0));

    let right = button(ui::icon(ui::glyph::CHEVRON_RIGHT, scale.px(28.0)))
        .on_press(Message::MetricsCategoryCycled(SlideDirection::Right))
        .style(arrow_style)
        .padding(scale.pad(0.0, 10.0));

    let title = text(category.label())
        .size(scale.text(22.0))
        .font(ui::semibold());

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

fn view_body(app: &App, scale: Scale) -> Element<'_, Message> {
    // The cards are sized by font + padding, so a category's stack can be
    // taller than the body's share of the panel (QuickView already is on
    // shorter windows). Instead of scrolling or clipping, measure the height
    // the body actually gets via `responsive` and compose an extra
    // shrink-to-fit factor into the scale so every card is always fully
    // visible. Each category fits independently — a tall QuickView stack
    // doesn't shrink the other categories' cards.
    responsive(move |body| {
        let fit = |cat: MetricsCategory| {
            (body.height / (natural_body_height(cat) * scale.factor())).min(1.0)
        };
        let current = view_category(app, app.metrics_category, scale.and(fit(app.metrics_category)));
        let (previous, direction, progress) = match app.metrics_transition {
            Some(t) => (
                Some(view_category(app, t.from, scale.and(fit(t.from)))),
                t.direction,
                t.progress(),
            ),
            None => (None, SlideDirection::Right, 1.0),
        };
        slide(current, previous, progress, direction).into()
    })
    .into()
}

fn view_category(app: &App, category: MetricsCategory, scale: Scale) -> Element<'_, Message> {
    match category {
        MetricsCategory::Daily => view_daily(app, scale),
        MetricsCategory::Session => view_session(app, scale),
        MetricsCategory::AllTime => view_all_time(app, scale),
        MetricsCategory::QuickView => view_quick(app, scale),
    }
}

fn view_daily(app: &App, scale: Scale) -> Element<'_, Message> {
    let m = &app.metrics;
    let tracked = m.tracked_duration_today();
    let primary = primary_card(
        ui::glyph::CLOCK,
        "TOTAL TIME TODAY",
        dash_if_untracked(tracked, fmt_duration_long(tracked)),
        if tracked.is_zero() { T1 } else { GREEN },
        scale,
    );

    let row_a = row![
        secondary_card(
            ui::glyph::TRIANGLE,
            "Breaks",
            dash_if_untracked(tracked, m.breaks_today().to_string()),
            false,
            scale,
        ),
        secondary_card(
            ui::glyph::CROSS,
            "Bad time",
            dash_if_untracked(tracked, fmt_duration_long(m.bad_posture_duration_today())),
            true,
            scale,
        ),
    ]
    .spacing(scale.px(8.0));

    let streak = m.good_posture_streak();
    let row_b = row![
        secondary_card(
            if streak.is_some() { ui::glyph::DISC } else { ui::glyph::CIRCLE },
            "Streak",
            fmt_duration(streak),
            false,
            scale,
        ),
        secondary_card(
            ui::glyph::HALF_DISC,
            "Since break",
            fmt_duration(m.time_since_last_break()),
            false,
            scale,
        ),
    ]
    .spacing(scale.px(8.0));

    column![primary, row_a, row_b]
        .spacing(scale.px(8.0))
        .width(Length::Fill)
        .into()
}

fn view_session(app: &App, scale: Scale) -> Element<'_, Message> {
    let m = &app.metrics;
    let session_active = m.is_session_active();
    let tracked = m.tracked_duration_session();

    let primary_value = if session_active {
        fmt_duration_long(tracked)
    } else {
        "--".to_string()
    };

    let primary = primary_card(
        ui::glyph::CLOCK,
        "SESSION LENGTH",
        primary_value,
        if session_active { GREEN } else { T1 },
        scale,
    );

    let row_a = row![
        secondary_card(
            ui::glyph::TRIANGLE,
            "Breaks",
            dash_if_untracked(tracked, m.breaks_session().to_string()),
            false,
            scale,
        ),
        secondary_card(
            ui::glyph::CROSS,
            "Bad time",
            dash_if_untracked(tracked, fmt_duration_long(m.bad_posture_duration_session())),
            true,
            scale,
        ),
    ]
    .spacing(scale.px(8.0));

    // No tracked time yet => show a neutral "--" with no progress fill, matching
    // the other empty-state cards instead of an earned-looking "100%".
    let quality_trailing: Element<'_, Message> = match m.posture_quality_session() {
        Some(quality) => row![
            progress_bar(0.0..=1.0, quality)
                .length(scale.px(100.0))
                .girth(scale.px(8.0)),
            text(format!("{:.0}%", quality * 100.0))
                .size(scale.text(16.0))
                .font(ui::semibold())
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(scale.px(10.0))
        .align_y(Alignment::Center)
        .into(),
        None => text("--")
            .size(scale.text(16.0))
            .font(ui::semibold())
            .color(T1)
            .wrapping(iced::widget::text::Wrapping::None)
            .into(),
    };

    let quality_card = container(
        row![
            row![
                ui::icon(ui::glyph::BARS, scale.px(13.0)),
                text("Posture Quality")
                    .size(scale.text(12.0))
                    .color(Color { a: 0.75, ..T1 }),
            ]
            .spacing(scale.px(6.0))
            .align_y(Alignment::Center),
            Space::new().width(Length::Fill),
            quality_trailing,
        ]
        .spacing(scale.px(10.0))
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding(scale.pad(10.0, 12.0))
    .width(Length::Fill)
    .clip(true)
    .style(|_| container::Style {
        background: Some(Background::Color(ELEV)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    });

    column![primary, row_a, quality_card]
        .spacing(scale.px(8.0))
        .width(Length::Fill)
        .into()
}

fn view_all_time(app: &App, scale: Scale) -> Element<'_, Message> {
    let m = &app.metrics;

    let tracked = m.all_time_tracked_duration();
    let primary = primary_card(
        ui::glyph::CLOCK,
        "LIFETIME TRACKED",
        dash_if_untracked(tracked, fmt_duration_long(tracked)),
        if tracked.is_zero() { T1 } else { GREEN },
        scale,
    );

    let row_a = row![
        secondary_card(
            ui::glyph::TRIANGLE,
            "Lifetime breaks",
            dash_if_untracked(tracked, m.all_time_breaks().to_string()),
            false,
            scale,
        ),
        secondary_card(
            ui::glyph::CROSS,
            "Bad time",
            dash_if_untracked(tracked, fmt_duration_long(m.all_time_bad_posture_duration())),
            true,
            scale,
        ),
    ]
    .spacing(scale.px(8.0));

    let days = m.all_time_days_tracked();
    let avg_breaks = if days > 0 {
        format!("{:.1}", m.all_time_breaks() as f32 / days as f32)
    } else {
        "--".to_string()
    };

    let row_b = row![
        secondary_card(
            ui::glyph::GRID,
            "Days tracked",
            dash_if_untracked(tracked, days.to_string()),
            false,
            scale,
        ),
        secondary_card(ui::glyph::TREND_UP, "Avg breaks/day", avg_breaks, false, scale),
    ]
    .spacing(scale.px(8.0));

    column![primary, row_a, row_b]
        .spacing(scale.px(8.0))
        .width(Length::Fill)
        .into()
}

fn view_quick(app: &App, scale: Scale) -> Element<'_, Message> {
    let m = &app.metrics;
    let tracked = m.tracked_duration_today();
    let untracked = tracked.is_zero();

    let streak = m.good_posture_streak();
    let streak_ok = streak.is_some();
    let streak_card = quick_card(
        if streak_ok { ui::glyph::DISC } else { ui::glyph::CIRCLE },
        "STREAK",
        fmt_duration(streak),
        None,
        if untracked { T1 } else if streak_ok { GREEN } else { RED },
        scale,
    );

    let breaks = m.breaks_today();
    let breaks_card = quick_card(
        ui::glyph::TRIANGLE,
        "BREAKS TODAY",
        dash_if_untracked(tracked, breaks.to_string()),
        None,
        if untracked { T1 } else if breaks == 0 { GREEN } else { RED },
        scale,
    );

    // Quality is `None` before anything is tracked: render "--" with no progress
    // fill and a neutral color rather than a misleading "100%".
    let quality = m.posture_quality_today();
    let quality_color = match quality {
        Some(q) if q >= 0.8 => GREEN,
        Some(q) if q >= 0.5 => T1,
        Some(_) => RED,
        None => T1,
    };
    let quality_card = quick_card(
        ui::glyph::BARS,
        "POSTURE QUALITY",
        quality.map_or_else(|| "--".to_string(), |q| format!("{:.0}%", q * 100.0)),
        quality,
        quality_color,
        scale,
    );

    column![streak_card, breaks_card, quality_card]
        .spacing(scale.px(10.0))
        .width(Length::Fill)
        .into()
}

fn primary_card<'a>(
    glyph: &'a str,
    label: &'a str,
    value: String,
    accent: Color,
    scale: Scale,
) -> Element<'a, Message> {
    let label_block = row![
        ui::icon(glyph, scale.px(18.0)),
        text(label)
            .size(scale.text(12.0))
            .font(ui::bold())
            .color(Color { a: 0.75, ..T1 }),
    ]
    .spacing(scale.px(8.0))
    .align_y(Alignment::Center);

    let value_text = text(value)
        .size(scale.text(23.0))
        .font(ui::semibold())
        .wrapping(iced::widget::text::Wrapping::None);

    let card = container(
        row![
            label_block,
            Space::new().width(Length::Fill),
            value_text,
        ]
        .spacing(scale.px(12.0))
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding(scale.pad(12.0, 14.0))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(HOVER)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    });

    let stripe = container(Space::new().width(scale.px(4.0)).height(Length::Fill))
        .style(move |_| container::Style {
            background: Some(Background::Color(accent)),
            border: Border::default().rounded(2),
            ..Default::default()
        });

    row![stripe, card]
        .spacing(scale.px(6.0))
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .into()
}

fn secondary_card<'a>(
    glyph: &'a str,
    label: &'a str,
    value: String,
    is_bad: bool,
    scale: Scale,
) -> Element<'a, Message> {
    let value_color = if is_bad {
        ui::mix(RED, Color::WHITE, 0.05)
    } else {
        T1
    };

    container(
        row![
            row![
                ui::icon(glyph, scale.px(13.0)).color(T1),
                text(label).size(scale.text(12.0)).color(T1),
            ]
            .spacing(scale.px(6.0))
            .align_y(Alignment::Center),
            Space::new().width(Length::Fill),
            text(value)
                .size(scale.text(17.0))
                .font(ui::semibold())
                .color(value_color)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(scale.px(10.0))
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding(scale.pad(10.0, 12.0))
    .width(Length::Fill)
    .clip(true)
    .style(|_| container::Style {
        background: Some(Background::Color(ELEV)),
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
    scale: Scale,
) -> Element<'a, Message> {
    let label_block = row![
        ui::icon(glyph, scale.px(18.0)).color(accent),
        text(label)
            .size(scale.text(12.0))
            .font(ui::bold())
            .color(Color { a: 0.75, ..T1 }),
    ]
    .spacing(scale.px(8.0))
    .align_y(Alignment::Center);

    let value_text = text(value)
        .size(scale.text(24.0))
        .font(ui::semibold())
        .wrapping(iced::widget::text::Wrapping::None);

    let inner: Element<'_, Message> = if let Some(p) = progress {
        row![
            label_block,
            Space::new().width(Length::Fill),
            progress_bar(0.0..=1.0, p)
                .length(scale.px(120.0))
                .girth(scale.px(10.0)),
            value_text,
        ]
        .spacing(scale.px(10.0))
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
    } else {
        row![
            label_block,
            Space::new().width(Length::Fill),
            value_text,
        ]
        .spacing(scale.px(12.0))
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
    };

    let card = container(inner)
        .padding(scale.pad(12.0, 14.0))
        .width(Length::Fill)
        .clip(true)
        .style(|_| container::Style {
            background: Some(Background::Color(HOVER)),
            border: Border {
                color: LINE,
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        });

    let stripe = container(Space::new().width(scale.px(4.0)).height(Length::Fill))
        .style(move |_| container::Style {
            background: Some(Background::Color(accent)),
            border: Border::default().rounded(2),
            ..Default::default()
        });

    row![stripe, card]
        .spacing(scale.px(6.0))
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .into()
}

fn view_footer(scale: Scale) -> Element<'static, Message> {
    let reset_btn = button(
        row![
            ui::icon(ui::glyph::REFRESH, scale.px(18.0)),
            text("Reset").size(scale.text(13.0)).font(ui::bold()),
        ]
        .spacing(scale.px(6.0))
        .align_y(Alignment::Center),
    )
    .on_press(Message::MetricsResetMenuToggled)
        .padding(scale.pad(4.0, 10.0))
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Color { a: 0.6, ..PANEL }
                }
                _ => Color { a: 0.3, ..PANEL },
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: T1,
                border: Border::default().rounded(6),
                ..button::Style::default()
            }
        });

    container(row![Space::new().width(Length::Fill), reset_btn].align_y(Alignment::Center))
        .width(Length::Fill)
        .height(Length::Fixed(scale.px(FOOTER_HEIGHT)))
        .align_y(Alignment::Center)
        .into()
}

fn reset_popup(category: MetricsCategory, scale: Scale) -> Element<'static, Message> {
    let cancel = button(text("Cancel").size(scale.text(13.0)))
        .on_press(Message::MetricsResetMenuToggled)
        .padding(scale.pad(4.0, 10.0))
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => ELEV,
                _ => PANEL,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: T1,
                border: Border::default().rounded(4),
                ..button::Style::default()
            }
        });

    let confirm = button(text("Reset").size(scale.text(13.0)).font(ui::semibold()))
        .on_press(Message::MetricsResetConfirmed)
        .padding(scale.pad(4.0, 10.0))
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => ui::mix(RED, Color::WHITE, 0.05),
                _ => RED,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: T1,
                border: Border::default().rounded(4),
                ..button::Style::default()
            }
        });

    container(
        column![
            text(format!("Reset {}?", category.label()))
                .size(scale.text(13.0))
                .font(ui::semibold()),
            row![cancel, confirm].spacing(scale.px(8.0)),
        ]
        .spacing(scale.px(8.0)),
    )
    .padding(scale.pad_all(10.0))
    .style(|_| container::Style {
        background: Some(Background::Color(PANEL)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    })
    .into()
}
