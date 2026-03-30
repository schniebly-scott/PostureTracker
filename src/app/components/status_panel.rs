use iced::widget::{column, container, row, slider, text};
use iced::{Background, Element};

use crate::app::theme::{DARK_BLUE, WARNING_RED};
use crate::app::{App, Message};

pub fn view(app: &App) -> Element<'_, Message> {
    let posture_state = if app.bad_posture {
        "Bad posture detected"
    } else if app.posture_angle_deg.is_some() {
        "Posture within threshold"
    } else {
        "Waiting for pose data"
    };

    let current_angle = app
        .posture_angle_deg
        .map(|angle| format!("{angle:.1} deg"))
        .unwrap_or_else(|| "--".to_string());

    let baseline_angle = app
        .posture_baseline_deg
        .map(|angle| format!("{angle:.1} deg"))
        .unwrap_or_else(|| "--".to_string());

    let delta_angle = match (app.posture_baseline_deg, app.posture_angle_deg) {
        (Some(baseline), Some(current)) => format!("{:.1} deg", (current - baseline).abs()),
        _ => "--".to_string(),
    };

    let slider_row = row![
        text("Angle Threshold"),
        slider(
            1.0..=45.0,
            app.posture_threshold_deg,
            Message::PostureThresholdChanged
        )
        .step(0.5)
        .width(iced::Length::Fill),
        text(format!("{:.1} deg", app.posture_threshold_deg)),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);

    let debug_stats = column![
        row![text("State:"), text(posture_state)].spacing(10),
        row![text("Current Angle:"), text(current_angle)].spacing(10),
        row![text("Baseline Angle:"), text(baseline_angle)].spacing(10),
        row![text("Angle Delta:"), text(delta_angle)].spacing(10),
        slider_row,
    ]
    .spacing(10);

    container(column![text("Debug Stats").size(20), debug_stats,].spacing(10))
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
        .height(iced::Length::Fill)
        .into()
}
