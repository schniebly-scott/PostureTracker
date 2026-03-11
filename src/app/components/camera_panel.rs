use iced::{Element, Length::Fill};
use iced::widget::{container, image, stack};

use crate::app::{App, Message};

pub fn view(app: &App) -> Element<'_, Message> {
    let img: Element<_> = match (&app.cam_frame, &app.cv_frame) {
        (Some(cam), Some(cv)) => stack![
            image(cam.clone()).width(Fill).height(Fill),
            image(cv.clone()).width(Fill).height(Fill),
        ]
        .into(),

        (Some(cam), None) => image(cam.clone())
            .width(Fill)
            .height(Fill)
            .into(),

        _ => container("Camera not started")
            .center_x(Fill)
            .center_y(Fill)
            .into(),
    };

    container(img)
        .width(iced::Length::FillPortion(3))
        .into()
}