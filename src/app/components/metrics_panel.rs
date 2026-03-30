use iced::widget::{column, container, row, text};
use iced::{Background, Element};

use crate::app::theme::MID_BLUE;
use crate::app::{App, Message};

pub fn view(app: &App) -> Element<'_, Message> {
    let model_load = app
        .model_load_time
        .map(|t| format!("{:?}", t))
        .unwrap_or_else(|| "Not loaded".to_string());

    let inference = app
        .inference_time
        .map(|t| format!("{:?}", t))
        .unwrap_or_else(|| "No inference".to_string());

    container(
        column![
            text("Metrics").size(20),
            row![text("Model Load:"), text(model_load)],
            row![text("Inference:"), text(inference)],
        ]
        .spacing(10),
    )
    .style(|_| container::Style {
        background: Some(Background::Color(MID_BLUE)),
        ..Default::default()
    })
    .padding(15)
    .width(iced::Length::Fill)
    .height(iced::Length::Fill)
    .into()
}
