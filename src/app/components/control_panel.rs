use iced::widget::{button, column, container, text};
use iced::{Background, Element};

use crate::app::theme::DARK_BLUE;
use crate::app::{App, InferenceState, Message};

pub fn view(app: &App) -> Element<'_, Message> {
    let debug_button = if app.debug_window_id.is_some() {
        button("Debug Window")
    } else {
        button("Debug Window").on_press(Message::OpenDebugWindowPressed)
    };

    let load_button = match app.inference_state {
        InferenceState::Running => button("Load Model"),
        InferenceState::Stopped | InferenceState::Unloaded => {
            button("Load Model").on_press(Message::LoadModelPressed)
        }
    };

    let control_button = match app.inference_state {
        InferenceState::Running => button("Stop Model").on_press(Message::StopInferencePressed),

        InferenceState::Stopped => button("Start Model").on_press(Message::StartInferencePressed),

        InferenceState::Unloaded => button("Start Model"),
    };

    container(
        column![
            text("Model Controls").size(20),
            load_button,
            control_button,
            debug_button
        ]
        .spacing(10),
    )
        .style(|_| container::Style {
            background: Some(Background::Color(DARK_BLUE)),
            ..Default::default()
        })
        .padding(15)
        .into()
}
