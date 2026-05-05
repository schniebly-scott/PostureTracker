use iced::widget::{column, container, mouse_area, text};
use iced::{Background, Element, Length::Fill};

use crate::app::theme::WARNING_RED;
use crate::app::{App, Message};

pub fn view(_app: &App) -> Element<'_, Message> {
    let content = column![
        text("BAD POSTURE DETECTED").size(72),
        text("Correct your posture, then click to dismiss").size(24),
    ]
    .align_x(iced::Alignment::Center)
    .spacing(30);

    mouse_area(
        container(content)
            .style(|_| container::Style {
                background: Some(Background::Color(WARNING_RED)),
                ..Default::default()
            })
            .center_x(Fill)
            .center_y(Fill)
            .width(Fill)
            .height(Fill),
    )
    .on_press(Message::DismissAlert)
    .into()
}
