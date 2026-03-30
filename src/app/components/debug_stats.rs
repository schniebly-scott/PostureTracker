use iced::widget::{column, container, row, text};
use iced::{Background, Element};

use crate::app::theme::{DARK_BLUE, WARNING_RED};
use crate::app::{App, Message};
pub fn view(app: &App) -> Element<'_, Message> {
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

    let debug_stats = column![
        row![text("Current Angle:"), text(current_angle)].spacing(10),
        row![text("Baseline Angle:"), text(baseline_angle)].spacing(10),
        row![text("Angle Delta:"), text(delta_angle)].spacing(10),
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