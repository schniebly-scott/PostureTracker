use iced::widget::{column, container, row, text};
use iced::{Background, Element};

use crate::app::theme::MID_BLUE;
use crate::app::{App, Message};

pub fn view(app: &App) -> Element<'_, Message> {
    container(
        column![
            text("Metrics").size(20),
            row![text("Breaks in Posture:"), text("TODO")],
            row![text("Time Since Last Break:"), text("TODO")],
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
