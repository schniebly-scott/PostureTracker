use iced::border::Border;
use iced::widget::{column, container, row, slider, text, text_input, toggler, Space};
use iced::{Alignment, Background, Color, Element, Length, Length::Fill};

use crate::app::components::debug_stats;
use crate::app::components::ui;
use crate::app::theme::{ELEV, GREEN, LINE, PANEL, RED, T1, T2, T3};
use crate::app::{App, Message, SampleIntervalChoice};

/// State of the live-status badge, mapped to a label + accent color.
fn status_state(app: &App) -> (&'static str, &'static str, Color) {
    let active = app.is_camera_running() || app.is_background_mode();
    if app.pipeline_error.is_some() {
        ("Camera unavailable", "bad", RED)
    } else if app.posture_baseline_deg.is_none() {
        ("Not calibrated", "neutral", T3)
    } else if app.bad_posture {
        ("Bad posture detected", "bad", RED)
    } else if app.posture_angle_deg.is_some() {
        ("Posture within range", "ok", GREEN)
    } else if active {
        ("Waiting for pose data", "neutral", T3)
    } else {
        ("Idle", "neutral", T3)
    }
}

fn state_badge<'a>(label: &'a str, kind: &str, color: Color) -> Element<'a, Message> {
    let (bg, border, text_color) = match kind {
        "neutral" => (ELEV, LINE, T2),
        _ => (ui::mix(PANEL, color, 0.15), ui::with_alpha(color, 0.38), color),
    };

    let dot = container(Space::new().width(9).height(9)).style(move |_| container::Style {
        background: Some(Background::Color(color)),
        border: Border::default().rounded(5),
        ..Default::default()
    });

    container(
        row![dot, text(label).size(13).font(ui::semibold()).color(text_color)]
            .spacing(9)
            .align_y(Alignment::Center),
    )
    .padding([7, 13])
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

fn state_line(app: &App) -> String {
    if let Some(error) = &app.pipeline_error {
        error.clone()
    } else if app.posture_baseline_deg.is_none() {
        "Calibrate a baseline to start checking your alignment.".to_string()
    } else if app.bad_posture {
        "You've drifted past your threshold — straighten up to clear the alert.".to_string()
    } else if let (Some(b), Some(a)) = (app.posture_baseline_deg, app.posture_angle_deg) {
        format!(
            "Looking good — your head-to-shoulder angle is {:.1}\u{00B0} from your calibrated baseline.",
            (a - b).abs()
        )
    } else if app.is_camera_running() || app.is_background_mode() {
        "Waiting for pose data — make sure your head and shoulders are visible.".to_string()
    } else {
        "Tracking is idle. Your alignment will appear here once a session begins.".to_string()
    }
}

/// Custom-track slider: green fill (red when bad), white knob, value read-out.
fn threshold_field(app: &App) -> Element<'_, Message> {
    let bad = app.bad_posture;
    let accent = if bad { RED } else { GREEN };

    let head = row![
        text("Angle threshold").size(16).font(ui::semibold()).color(T2),
        Space::new().width(Fill),
        ui::value(format!("{:.1}\u{00B0}", app.posture_threshold_deg), 13, T1),
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
            width: 6.0,
            border: Border {
                color: LINE,
                width: 1.0,
                radius: 999.0.into(),
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius: 9.0 },
            background: Background::Color(Color::WHITE),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    });

    column![head, track].spacing(9).into()
}

fn interval_field(app: &App) -> Element<'_, Message> {
    let sel = app.sample_interval_choice;
    let opts = [
        ("Constant", SampleIntervalChoice::Constant),
        ("30s", SampleIntervalChoice::Secs30),
        ("1 min", SampleIntervalChoice::Min1),
        ("5 min", SampleIntervalChoice::Min5),
        ("Custom", SampleIntervalChoice::Custom),
    ];

    let mut seg = row![].spacing(2);
    for (label, choice) in opts {
        seg = seg.push(ui::seg_button(
            label,
            sel == choice,
            Message::SampleIntervalChoiceChanged(choice),
        ));
    }

    let seg = container(seg)
        .padding(3)
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
        text("Check interval").size(16).font(ui::semibold()).color(T2),
        seg,
    ]
    .spacing(9);

    if sel == SampleIntervalChoice::Custom {
        col = col.push(
            row![
                text_input("min", &app.custom_interval_input)
                    .on_input(Message::CustomIntervalInputChanged)
                    .on_submit(Message::CommitConfig)
                    .width(70),
                text("minutes").size(12).color(T3),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }

    col.into()
}

fn dismiss_toggle(app: &App) -> Element<'_, Message> {
    let copy = column![
        text("Require manual dismissal").size(16).font(ui::semibold()).color(T2),
        text("Alerts stay until you acknowledge them").size(13).color(T3),
    ]
    .spacing(2);

    row![
        copy,
        Space::new().width(Fill),
        toggler(app.force_dismiss).on_toggle(Message::ForceDismissToggled),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn graph_card(app: &App) -> Element<'_, Message> {
    let legend = |color: Color, label: &'static str| {
        row![
            container(Space::new().width(14).height(3)).style(move |_| container::Style {
                background: Some(Background::Color(color)),
                border: Border::default().rounded(2),
                ..Default::default()
            }),
            text(label).size(16).color(T3),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
    };

    let head = row![
        text("HEAD-TO-SHOULDER ANGLE \u{00B7} LAST 2 MIN")
            .size(14)
            .font(ui::semibold())
            .color(T3),
        Space::new().width(Fill),
        legend(GREEN, "Angle"),
        legend(RED, "Threshold"),
    ]
    .spacing(14)
    .align_y(Alignment::Center);

    let chart: Element<_> = if app.metrics.angle_history.is_empty() {
        container(
            text("No data — start a session to graph your posture")
                .size(18)
                .color(T3),
        )
        .center_x(Fill)
        .center_y(Fill)
        .into()
    } else {
        debug_stats::angle_chart(app, Length::Fill)
    };

    container(column![head, chart].spacing(8).height(Fill))
        .padding([14, 16])
        // Larger share than the controls column (FillPortion(2)) to its left.
        .width(Length::FillPortion(3))
        .height(Fill)
        .style(ui::tile_style)
        .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let (label, kind, color) = status_state(app);

    let mut left = column![
        ui::micro_label("Live status"),
        state_badge(label, kind, color),
        text(state_line(app)).size(14).color(T2).width(Fill),
    ]
    .spacing(12);

    if app.pipeline_error.is_some() {
        left = left.push(
            row![
                ui::secondary_button(
                    text("Camera settings").size(13).font(ui::semibold()).into()
                )
                .on_press(Message::OpenSettingsPressed),
                ui::danger_button(text("Dismiss").size(13).font(ui::semibold()).into())
                    .on_press(Message::DismissPipelineError),
            ]
            .spacing(8),
        );
    }

    let left = left
        .push(threshold_field(app))
        .push(interval_field(app))
        .push(dismiss_toggle(app))
        .spacing(16)
    // Reflows with the window but stays narrow enough to keep the controls
    // readable; the graph card to its right takes the remaining width.
    .width(Length::FillPortion(2))
    .max_width(366.0);

    let body = row![left, graph_card(app)]
        .spacing(22)
        .height(Fill);

    container(body)
        .style(ui::panel_alert_style(
            app.bad_posture || app.pipeline_error.is_some(),
        ))
        .padding(16)
        .width(Fill)
        .height(Length::FillPortion(5))
        .into()
}
