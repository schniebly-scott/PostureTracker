use iced::widget::{button, column, container, pick_list, text};
use iced::{Background, Element};

use crate::app::theme::DARK_BLUE;
use crate::app::{App, CalibrationState, InferenceState, Message, SettingsOption};

pub fn view(app: &App) -> Element<'_, Message> {
    let test_button = match app.inference_state {
        InferenceState::Running => button("Stop Test").on_press(Message::StopInferencePressed),
        InferenceState::Stopped | InferenceState::Unloaded => {
            button("Test Posture").on_press(Message::TestPosturePressed)
        }
    };

    let calibrate_button = match &app.calibration_state {
        CalibrationState::Idle => {
            if app.inference_state == InferenceState::Running {
                button("Set Baseline").on_press(Message::CalibratePressed)
            } else {
                button("Set Baseline")
            }
        }
        CalibrationState::Countdown(n) => button(text(format!("Starting in {n}..."))),
        CalibrationState::Sampling { .. } => button("Sampling..."),
        CalibrationState::Failed(msg) => button(text(format!("Failed: {msg}"))),
    };

    let settings_pick_list = pick_list(
        app.settings_options(),
        None::<SettingsOption>,
        Message::SettingsOptionSelected,
    )
    .placeholder("\u{2699} Settings")
    .width(180)
    .menu_height(140)
    .padding([8, 12]);

    let tray_hint = if app.has_system_tray() {
        None
    } else {
        Some(text(app.background_action_hint()).size(12))
    };

    container(
        column![
            text("Model Controls").size(20),
            settings_pick_list,
            test_button,
            calibrate_button,
            tray_hint.map(Element::from).unwrap_or_else(|| container(text("")).into()),
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
