use iced::widget::{button, column, container, pick_list, text};
use iced::{Background, Element};

use crate::app::theme::DARK_BLUE;
use crate::app::{App, CalibrationState, InferenceState, Message, SettingsOption};

pub fn view(app: &App) -> Element<'_, Message> {
    let settings_pick_list = pick_list(
        app.settings_options(),
        None::<SettingsOption>,
        Message::SettingsOptionSelected,
    )
    .placeholder("\u{2699} Settings")
    .width(180)
    .menu_height(140)
    .padding([8, 12]);

    let content = if app.is_background_mode() {
        column![
            text("Model Controls").size(20),
            settings_pick_list,
            button("Stop Background Tracking").on_press(Message::StopBackgroundPressed),
        ]
        .spacing(10)
    } else {
        let test_button = match app.inference_state {
            InferenceState::Running => button("Stop Test").on_press(Message::StopInferencePressed),
            InferenceState::Stopped | InferenceState::Unloaded => {
                button("Test Posture").on_press(Message::TestPosturePressed)
            }
        };

        let calibrate_button = if app.inference_state == InferenceState::Running {
            let btn = match &app.calibration_state {
                CalibrationState::Idle => {
                    let label = if app.is_calibrated() { "Re-Calibrate" } else { "Set Baseline" };
                    button(label).on_press(Message::CalibratePressed)
                }
                CalibrationState::Countdown(n) => button(text(format!("Starting in {n}..."))),
                CalibrationState::Sampling { .. } => button("Sampling..."),
                CalibrationState::Failed(msg) => button(text(format!("Failed: {msg}"))),
            };
            Some(btn)
        } else {
            None
        };

        let enter_bg_button = if app.is_calibrated()
            && matches!(app.calibration_state, CalibrationState::Idle)
        {
            Some(button("Enter Background").on_press(Message::EnterBackgroundPressed))
        } else {
            None
        };

        let mut col = column![
            text("Model Controls").size(20),
            settings_pick_list,
            test_button,
        ]
        .spacing(10);

        if let Some(btn) = calibrate_button {
            col = col.push(btn);
        }
        if let Some(btn) = enter_bg_button {
            col = col.push(btn);
        }

        col
    };

    container(content)
        .style(|_| container::Style {
            background: Some(Background::Color(DARK_BLUE)),
            ..Default::default()
        })
        .padding(15)
        .into()
}
