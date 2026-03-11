use iced::{Element};
use iced::widget::{column, row, button, text, container};

use crate::app::{App, Message, InferenceState};

pub fn view(app: &App) -> Element<'_, Message> {

    let load_button = match app.inference_state {
        InferenceState::Running => button("Load Model"),
        InferenceState::Stopped | InferenceState::Unloaded =>
            button("Load Model").on_press(Message::LoadModelPressed),
    };

    let control_button = match app.inference_state {
        InferenceState::Running =>
            button("Stop Model").on_press(Message::StopInferencePressed),

        InferenceState::Stopped =>
            button("Start Model").on_press(Message::StartInferencePressed),

        InferenceState::Unloaded =>
            button("Start Model"),
    };

    container(
        column![
            text("Model Controls").size(20),
            row![load_button, control_button].spacing(10),
        ]
        .spacing(10)
    )
    .padding(15)
    .into()
}