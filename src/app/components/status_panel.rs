use iced::border::Border;
use iced::widget::{column, container, row, slider, text, text_input, toggler, Space};
use iced::{Alignment, Background, Color, Element, Length, Length::Fill};

use crate::app::components::debug_stats;
use crate::app::components::ui;
use crate::app::components::ui::Scale;
use crate::app::theme::{ELEV, GREEN, LINE, PANEL, RED, T1, T2, T3};
use crate::app::{App, Message, SampleIntervalChoice};
use crate::metrics::HISTORY_SECS;

const STATUS_PANEL_MAX_WIDTH: f32 = 1072.0;

#[derive(Clone, Copy)]
enum BadgeKind {
    Neutral,
    Ok,
    Bad,
}

impl BadgeKind {
    fn color(self) -> Color {
        match self {
            Self::Neutral => T3,
            Self::Ok => GREEN,
            Self::Bad => RED,
        }
    }
}

/// State of the live-status badge and its matching explanatory sentence.
fn live_status(app: &App) -> (BadgeKind, &'static str, String) {
    let active = app.is_camera_running() || app.is_background_mode();
    if let Some(error) = &app.pipeline_error {
        // A pipeline failure outranks every other status: whatever the badge
        // would otherwise claim, tracking isn't happening.
        return (BadgeKind::Bad, "Tracking error", error.clone());
    }
    if app.posture_baseline_deg.is_none() {
        (
            BadgeKind::Neutral,
            "Not calibrated",
            "Calibrate a baseline to start checking your alignment.".to_string(),
        )
    } else if app.bad_posture {
        (
            BadgeKind::Bad,
            "Bad posture detected",
            "You've drifted past your threshold — straighten up to clear the alert.".to_string(),
        )
    } else if let (Some(b), Some(a)) = (app.posture_baseline_deg, app.posture_angle_deg) {
        (
            BadgeKind::Ok,
            "Posture within range",
            format!(
                "Looking good — your head-to-shoulder angle is {:.1}\u{00B0} from your calibrated baseline.",
                (a - b).abs()
            ),
        )
    } else if active {
        (
            BadgeKind::Neutral,
            "Waiting for pose data",
            "Waiting for pose data — make sure your head and shoulders are visible.".to_string(),
        )
    } else {
        (
            BadgeKind::Neutral,
            "Idle",
            "Tracking is idle. Your alignment will appear here once a session begins.".to_string(),
        )
    }
}

fn state_badge<'a>(label: &'a str, kind: BadgeKind, scale: Scale) -> Element<'a, Message> {
    let color = kind.color();
    let (bg, border, text_color) = match kind {
        BadgeKind::Neutral => (ELEV, LINE, T2),
        BadgeKind::Ok | BadgeKind::Bad => (
            ui::mix(PANEL, color, 0.15),
            ui::with_alpha(color, 0.38),
            color,
        ),
    };

    let dot = container(Space::new().width(scale.px(9.0)).height(scale.px(9.0))).style(
        move |_| container::Style {
            background: Some(Background::Color(color)),
            border: Border::default().rounded(5),
            ..Default::default()
        },
    );

    container(
        row![
            dot,
            text(label)
                .size(scale.text(13.0))
                .font(ui::semibold())
                .color(text_color)
        ]
        .spacing(scale.px(9.0))
        .align_y(Alignment::Center),
    )
    .padding(scale.pad(7.0, 13.0))
    .style(move |_| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: border,
            width: 1.0,
            radius: 999.0.into(),
        },
        ..Default::default()
    })
    .into()
}

/// Custom-track slider: green fill (red when bad), white knob, value read-out.
fn threshold_field(app: &App, scale: Scale) -> Element<'_, Message> {
    let bad = app.bad_posture;
    let accent = if bad { RED } else { GREEN };

    let head = row![
        text("Angle threshold")
            .size(scale.text(16.0))
            .font(ui::semibold())
            .color(T2),
        Space::new().width(Fill),
        ui::value(
            format!("{:.1}\u{00B0}", app.posture_threshold_deg),
            scale.text(13.0),
            T1,
        ),
    ]
    .align_y(Alignment::Center);

    let track = slider(
        1.0..=45.0,
        app.posture_threshold_deg,
        Message::PostureThresholdChanged,
    )
    .step(0.5)
    .width(Fill)
    .on_release(Message::PostureThresholdReleased)
    .style(move |_theme, _status| slider::Style {
        rail: slider::Rail {
            backgrounds: (
                Background::Color(accent),
                Background::Color(ELEV),
            ),
            width: scale.px(6.0),
            border: Border {
                color: LINE,
                width: 1.0,
                radius: 999.0.into(),
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle {
                radius: scale.px(9.0),
            },
            background: Background::Color(Color::WHITE),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    });

    column![head, track].spacing(scale.px(9.0)).into()
}

fn interval_field(app: &App, scale: Scale) -> Element<'_, Message> {
    let sel = app.sample_interval_choice;
    let opts = [
        ("Constant", SampleIntervalChoice::Constant),
        ("30s", SampleIntervalChoice::Secs30),
        ("1 min", SampleIntervalChoice::Min1),
        ("5 min", SampleIntervalChoice::Min5),
        ("Custom", SampleIntervalChoice::Custom),
    ];

    let mut seg = row![].spacing(scale.px(2.0));
    for (label, choice) in opts {
        seg = seg.push(ui::seg_button(
            label,
            sel == choice,
            Message::SampleIntervalChoiceChanged(choice),
            scale,
        ));
    }

    let seg = container(seg)
        .padding(scale.pad_all(3.0))
        .style(|_| container::Style {
            background: Some(Background::Color(ELEV)),
            border: Border {
                color: LINE,
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        });

    let mut col = column![
        text("Check interval")
            .size(scale.text(16.0))
            .font(ui::semibold())
            .color(T2),
        seg,
    ]
    .spacing(scale.px(9.0));

    if sel == SampleIntervalChoice::Custom {
        col = col.push(
            row![
                text_input("min", &app.custom_interval_input)
                    .on_input(Message::CustomIntervalInputChanged)
                    .on_submit(Message::CommitConfig)
                    .size(scale.text(16.0))
                    .width(scale.px(70.0)),
                text("minutes").size(scale.text(12.0)).color(T3),
            ]
            .spacing(scale.px(8.0))
            .align_y(Alignment::Center),
        );
    }

    col.into()
}

fn dismiss_toggle(app: &App, scale: Scale) -> Element<'_, Message> {
    let copy = column![
        text("Require manual dismissal")
            .size(scale.text(16.0))
            .font(ui::semibold())
            .color(T2),
        text("Alerts stay until you acknowledge them")
            .size(scale.text(13.0))
            .color(T3),
    ]
    .spacing(scale.px(2.0));

    row![
        copy,
        Space::new().width(Fill),
        toggler(app.force_dismiss)
            .size(scale.px(16.0))
            .on_toggle(Message::ForceDismissToggled),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn graph_card(app: &App, scale: Scale) -> Element<'_, Message> {
    let history_label = format!(
        "HEAD-TO-SHOULDER ANGLE \u{00B7} LAST {:.0} MIN",
        HISTORY_SECS / 60.0
    );
    let legend = move |color: Color, label: &'static str| {
        row![
            container(Space::new().width(scale.px(14.0)).height(scale.px(3.0))).style(
                move |_| container::Style {
                    background: Some(Background::Color(color)),
                    border: Border::default().rounded(2),
                    ..Default::default()
                }
            ),
            text(label).size(scale.text(16.0)).color(T3),
        ]
        .spacing(scale.px(6.0))
        .align_y(Alignment::Center)
    };

    let head = row![
        text(history_label)
            .size(scale.text(14.0))
            .font(ui::semibold())
            .color(T3),
        Space::new().width(Fill),
        legend(GREEN, "Angle"),
        legend(RED, "Threshold"),
    ]
    .spacing(scale.px(14.0))
    .align_y(Alignment::Center);

    let chart: Element<_> = if app.metrics.angle_history.is_empty() {
        container(
            text("No data — start a session to graph your posture")
                .size(scale.text(18.0))
                .color(T3),
        )
        .center_x(Fill)
        .center_y(Fill)
        .into()
    } else {
        debug_stats::angle_chart(app, Length::Fill)
    };

    container(column![head, chart].spacing(scale.px(8.0)).height(Fill))
        .padding(scale.pad(14.0, 16.0))
        // Larger share than the controls column (FillPortion(2)) to its left.
        .width(Length::FillPortion(3))
        .height(Fill)
        .style(ui::tile_style)
        .into()
}

pub fn view(app: &App, scale: Scale) -> Element<'_, Message> {
    let (kind, label, status_line) = live_status(app);

    // When a pipeline error is displayed, pair the badge with a dismiss
    // button so the banner doesn't outlive its usefulness.
    let badge: Element<'_, Message> = if app.pipeline_error.is_some() {
        row![
            state_badge(label, kind, scale),
            ui::ghost_button(text("Dismiss").size(scale.text(13.0)).into(), scale)
                .padding(scale.pad(7.0, 13.0))
                .on_press(Message::DismissPipelineError),
        ]
        .spacing(scale.px(9.0))
        .align_y(Alignment::Center)
        .into()
    } else {
        state_badge(label, kind, scale)
    };

    let left = column![
        ui::micro_label("Live status", scale),
        badge,
        text(status_line).size(scale.text(14.0)).color(T2),
        threshold_field(app, scale),
        interval_field(app, scale),
        dismiss_toggle(app, scale),
    ]
    .spacing(scale.px(16.0))
    // Fixed width so the status controls keep a stable, readable size; the
    // graph card to its right is the row's only fill child, so it absorbs the
    // extra width. (A `max_width` on a FillPortion child is a no-op here:
    // iced's flex layout pins a fill main-axis child to min == max == its
    // portion, so the cap is clamped away.)
    .width(Length::Fixed(scale.px(366.0)));

    let body = row![left, graph_card(app, scale)]
        .spacing(scale.px(22.0))
        .height(Fill);

    container(body)
        .style(ui::panel_alert_style(app.bad_posture))
        .padding(scale.pad_all(16.0))
        // Width is the cross axis of the dashboard's vertical column here, so
        // (unlike a row's main axis) `max_width` is honored: the panel fills
        // the width up to this cap, then stops stretching on wide displays.
        .width(Fill)
        .max_width(STATUS_PANEL_MAX_WIDTH)
        // Takes the smaller share of the dashboard column; the top row above
        // is FillPortion(7).
        .height(Length::FillPortion(5))
        .into()
}
