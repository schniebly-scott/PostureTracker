use iced::font::Weight;
use iced::widget::{checkbox, column, container, radio, row, slider, text, text_input};
use iced::{Alignment, Background, Color, Element, Font, Length};

use crate::app::components::debug_stats;
use crate::app::theme::{ACTIVE_RED, DARK_BLUE, OWHITE, WARNING_RED};
use crate::app::{App, Message, SampleIntervalChoice};

pub fn view(app: &App) -> Element<'_, Message> {
    let posture_state = if app.posture_baseline_deg.is_none() {
        "Not calibrated"
    } else if app.bad_posture {
        "Bad posture detected"
    } else if app.posture_angle_deg.is_some() {
        "Posture within threshold"
    } else {
        "Waiting for pose data"
    };

    let slider_row = row![
        text("Angle Threshold"),
        slider(
            1.0..=45.0,
            app.posture_threshold_deg,
            Message::PostureThresholdChanged
        )
        .step(0.5)
        .width(iced::Length::Fill)
        .on_release(Message::PostureThresholdReleased),
        text(format!("{:.1} deg", app.posture_threshold_deg)),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);

    let selected = Some(app.sample_interval_choice);

    let interval_row = row![
        text("Check Interval:"),
        radio("Constant", SampleIntervalChoice::Constant, selected, Message::SampleIntervalChoiceChanged),
        radio("30 sec",   SampleIntervalChoice::Secs30,   selected, Message::SampleIntervalChoiceChanged),
        radio("1 min",    SampleIntervalChoice::Min1,     selected, Message::SampleIntervalChoiceChanged),
        radio("5 min",    SampleIntervalChoice::Min5,     selected, Message::SampleIntervalChoiceChanged),
        radio("Custom:",  SampleIntervalChoice::Custom,   selected, Message::SampleIntervalChoiceChanged),
        text_input("min", &app.custom_interval_input)
            .on_input(Message::CustomIntervalInputChanged)
            .width(60),
        text("min"),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);

    let dismiss_row = row![
        checkbox(app.force_dismiss)
            .label("Require manual dismissal of alerts")
            .on_toggle(Message::ForceDismissToggled),
    ];

    let bold = Font { weight: Weight::Bold, ..Font::default() };
    let mode_label = app.mode_label();
    let mode_color: Color = match mode_label {
        "Idle" => Color { a: 0.5, ..OWHITE },
        _      => ACTIVE_RED,
    };
    let mode_font = match mode_label {
        "Idle" => Font::default(),
        _      => bold,
    };
    let mode_row = row![
        text("Mode:").font(bold).size(18),
        text(format!("● {mode_label}")).color(mode_color).font(mode_font).size(18),
    ]
    .spacing(8);

    container(
        column![
            text("Status").size(20),
            mode_row,
            row![text("State:"), text(posture_state)].spacing(10),
            slider_row,
            if app.metrics.angle_history.is_empty() {
                container(
                    text("No data yet — start testing to see the angle graph")
                        .color(Color { a: 0.4, ..OWHITE })
                        .size(16),
                )
                .width(Length::Fill)
                .height(160)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into()
            } else {
                debug_stats::angle_chart(app, 160u32)
            },
            interval_row,
            dismiss_row,
        ]
        .spacing(10),
    )
    .style(move |_| container::Style {
        background: Some(Background::Color(if app.bad_posture {
            WARNING_RED
        } else {
            DARK_BLUE
        })),
        ..Default::default()
    })
    .padding(15)
    .width(iced::Length::Fill)
    .into()
}
