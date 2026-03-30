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

    container(
        column![
            text("Status").size(20), 
            row![text("State:"), text(posture_state)].spacing(10),
            slider_row
        ].spacing(10)
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
