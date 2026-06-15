use iced::border::Border;
use iced::widget::{column, container, row, stack, text, tooltip};
use iced::{Alignment, Background, Element, Length, Length::Fill};

use crate::app::components::ui;
use crate::app::theme::{ELEV, LINE, T1};
use crate::app::{App, CalibrationState, InferenceState, Message, RunMode};

/// Fixed height the control panel reserves so the layout stays stable when the
/// calibrate / start buttons appear mid-session. The panel and its gear
/// overlay (in the same `stack!`) must share this value to stay aligned.
const PANEL_HEIGHT: f32 = 210.0;

/// A button label: a leading glyph plus text, sized for the button system.
fn btn_label(glyph: &str, label: &str) -> Element<'static, Message> {
    glyph_label(glyph, 14, label)
}

/// Like [`btn_label`] but with an explicit glyph size — used for the icon
/// glyphs that need to read larger than the surrounding text.
///
/// Both texts share a fixed line height so the label's vertical extent is
/// identical no matter which glyph is used; that keeps every button the same
/// height (otherwise e.g. ■ and ▶ would size their buttons differently).
/// The whole label is centered horizontally for a balanced full-width button.
fn glyph_label(glyph: &str, glyph_size: u16, label: &str) -> Element<'static, Message> {
    let line = text::LineHeight::Absolute(20.0.into());
    container(
        row![
            text(glyph.to_string()).size(glyph_size as f32).line_height(line),
            text(label.to_string())
                .size(14)
                .font(ui::semibold())
                .line_height(line),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .center_x(Fill)
    .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let content = if app.is_background_mode() {
        column![
            ui::micro_label("Session"),
            ui::danger_button(btn_label("\u{25A0}", "Stop Tracking"))
                .width(Fill)
                .on_press(Message::StopBackgroundPressed),
        ]
        .spacing(12)
    } else {
        // Slot 1 — the primary session action.
        let test_button = match app.inference_state {
            InferenceState::Running => ui::danger_button(btn_label("\u{25A0}", "Stop Test"))
                .on_press(Message::StopInferencePressed),
            InferenceState::Stopped | InferenceState::Unloaded => {
                ui::secondary_button(btn_label("\u{25B6}", "Test Posture"))
                    .on_press(Message::TestPosturePressed)
            }
        };

        // Slot 2 — calibration. Always visible; disabled while no test runs.
        let calibrate_label = if app.is_calibrated() {
            "Re-Calibrate"
        } else {
            "Set Baseline"
        };
        let calibrate_button: Element<'_, Message> = if app.inference_state
            == InferenceState::Running
        {
            match &app.calibration_state {
                CalibrationState::Idle => ui::ghost_button(btn_label("\u{25CE}", calibrate_label))
                    .width(Fill)
                    .on_press(Message::CalibratePressed)
                    .into(),
                CalibrationState::Countdown(n) => ui::disabled_button(
                    text(format!("Starting in {n}\u{2026}"))
                        .size(14)
                        .font(ui::semibold())
                        .into(),
                )
                .width(Fill)
                .into(),
                CalibrationState::Sampling { .. } => ui::disabled_button(
                    text("Sampling\u{2026}").size(14).font(ui::semibold()).into(),
                )
                .width(Fill)
                .into(),
                CalibrationState::Failed(msg) => ui::disabled_button(
                    text(format!("Failed: {msg}")).size(13).into(),
                )
                .width(Fill)
                .into(),
            }
        } else {
            // No test running — show the calibrate button greyed out (no on_press)
            // with a tooltip on hover explaining why it can't be used yet.
            let btn = ui::disabled_button(btn_label("\u{25CE}", calibrate_label)).width(Fill);
            tooltip(
                btn,
                container(text("Start a test to calibrate your baseline").size(13).color(T1))
                    .style(ui::tile_style)
                    .padding([6, 10]),
                tooltip::Position::Top,
            )
            .gap(6)
            .into()
        };

        let start_row = if app.is_calibrated()
            && matches!(app.calibration_state, CalibrationState::Idle)
        {
            // Segmented toggle choosing how the session starts (matches the
            // interval control in the settings panel).
            let toggle = container(
                row![
                    ui::seg_button(
                        "Foreground",
                        app.session_start_mode == RunMode::Foreground,
                        Message::SessionModeSelected(RunMode::Foreground),
                    ),
                    ui::seg_button(
                        "Background",
                        app.session_start_mode == RunMode::Background,
                        Message::SessionModeSelected(RunMode::Background),
                    ),
                ]
                .spacing(2),
            )
            .padding(3)
            .style(|_| container::Style {
                background: Some(Background::Color(ELEV)),
                border: Border {
                    color: LINE,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            });

            // Start Session dispatches whichever start the toggle selected.
            let start_msg = match app.session_start_mode {
                RunMode::Background => Message::EnterBackgroundPressed,
                RunMode::Foreground => Message::StartForegroundPressed,
            };

            Some(
                row![
                    toggle,
                    ui::primary_button(btn_label("\u{25B6}", "Start Session"))
                        .width(Fill)
                        .on_press(start_msg),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
        } else {
            None
        };

        let mut col = column![ui::micro_label("Session")]
            .spacing(10)
            .height(Fill);

        // Top row — the two start buttons, when available.
        if let Some(r) = start_row {
            col = col.push(r);
        }

        // Full-width, centered start-test button below the start row.
        col = col.push(test_button.width(Fill));

        // Calibrate button mirrors the test button's full-width formatting.
        col = col.push(calibrate_button);

        col
    };

    let panel = container(content)
        .style(ui::panel_style)
        .padding(16)
        // Fixed height so the panel reserves room for the calibrate / start
        // buttons that appear mid-session, keeping the layout stable.
        .height(Length::Fixed(PANEL_HEIGHT))
        .width(Fill);

    // Settings gear hovers in the panel's top-right corner.
    let gear = 
                container(tooltip(ui::icon_button("\u{2699}", 20).on_press(Message::OpenSettingsPressed),
                container(text("Settings").size(15).color(T1))
                    .style(ui::tile_style)
                    .padding([3, 10]),
                tooltip::Position::Bottom,
            )
            .gap(2))
        .align_x(Alignment::End)
        .align_y(Alignment::Start)
        .width(Fill)
        .height(Length::Fixed(PANEL_HEIGHT))
        .padding(5);
    

    stack![panel, gear].into()
}
