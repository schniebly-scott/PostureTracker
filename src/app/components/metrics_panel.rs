use iced::{Element};
use iced::widget::{column, row, text, container};

use crate::app::{App, Message};

pub fn view(app: &App) -> Element<'_, Message> {

    let model_load = app.model_load_time
        .map(|t| format!("{:?}", t))
        .unwrap_or_else(|| "Not loaded".to_string());

    let inference = app.inference_time
        .map(|t| format!("{:?}", t))
        .unwrap_or_else(|| "No inference".to_string());

    container(
        column![
            text("Metrics").size(20),
            row![text("Model Load:"), text(model_load)],
            row![text("Inference:"), text(inference)],
        ]
        .spacing(10)
    )
    .padding(15)
    .into()
}