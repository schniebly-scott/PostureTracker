use iced::border::Border;
use iced::widget::{column, container, row, stack, text, tooltip};
use iced::{Alignment, Background, Element, Length, Length::Fill};

use crate::app::components::ui;
use crate::app::components::ui::Scale;
use crate::app::theme::{ELEV, LINE, T1};
use crate::app::{App, CalibrationState, InferenceState, Message, RunMode};

/// Fixed height the control panel reserves so the layout stays stable when the
/// calibrate / start buttons appear mid-session. The panel and its gear
/// overlay (in the same `stack!`) must share this value to stay aligned.
const PANEL_HEIGHT: f32 = 210.0;

/// A button label: a leading glyph plus text, sized for the button system.
fn btn_label<'a>(glyph: &'a str, label: &'a str, scale: Scale) -> Element<'a, Message> {
    glyph_label(glyph, 14.0, label, scale)
}

/// Like [`btn_label`] but with an explicit glyph size — used for the icon
/// glyphs that need to read larger than the surrounding text.
///
/// The glyph comes from the bundled icon font ([`ui::icon`]), so every icon
/// has the same advance and baseline on every OS. Both texts still share one
/// fixed line height so the label's vertical extent doesn't shift between the
/// icon and the Inter label, keeping every button the same height. The whole
/// label is centered horizontally for a balanced full-width button.
fn glyph_label<'a>(
    glyph: &'a str,
    glyph_size: f32,
    label: &'a str,
    scale: Scale,
) -> Element<'a, Message> {
    let line = text::LineHeight::Absolute(scale.px(20.0).into());
    container(
        row![
            ui::icon(glyph, scale.px(glyph_size)).line_height(line),
            text(label)
                .size(scale.text(14.0))
                .font(ui::semibold())
                .line_height(line),
        ]
        .spacing(scale.px(9.0))
        .align_y(Alignment::Center),
    )
    .center_x(Fill)
    .into()
}

pub fn view(app: &App, scale: Scale) -> Element<'_, Message> {
    let content = if app.is_background_mode() {
        column![
            ui::micro_label("Session", scale),
            ui::danger_button(btn_label(ui::glyph::STOP, "Stop Tracking", scale), scale)
                .width(Fill)
                .on_press(Message::StopBackgroundPressed),
        ]
        .spacing(scale.px(12.0))
    } else {
        // Slot 1 — the primary session action.
        let test_button = match app.inference_state {
            InferenceState::Running => {
                ui::danger_button(btn_label(ui::glyph::STOP, "Stop Test", scale), scale)
                    .on_press(Message::StopInferencePressed)
            }
            InferenceState::Stopped | InferenceState::Unloaded => {
                ui::secondary_button(btn_label(ui::glyph::PLAY, "Test Posture", scale), scale)
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
                CalibrationState::Idle => {
                    ui::ghost_button(btn_label(ui::glyph::TARGET, calibrate_label, scale), scale)
                        .width(Fill)
                        .on_press(Message::CalibratePressed)
                        .into()
                }
                CalibrationState::Countdown(n) => ui::disabled_button(
                    text(format!("Starting in {n}\u{2026}"))
                        .size(scale.text(14.0))
                        .font(ui::semibold())
                        .into(),
                    scale,
                )
                .width(Fill)
                .into(),
                CalibrationState::Sampling { .. } => ui::disabled_button(
                    text("Sampling\u{2026}")
                        .size(scale.text(14.0))
                        .font(ui::semibold())
                        .into(),
                    scale,
                )
                .width(Fill)
                .into(),
                CalibrationState::Failed(msg) => ui::disabled_button(
                    text(format!("Failed: {msg}")).size(scale.text(13.0)).into(),
                    scale,
                )
                .width(Fill)
                .into(),
            }
        } else {
            // No test running — show the calibrate button greyed out (no on_press)
            // with a tooltip on hover explaining why it can't be used yet.
            let btn =
                ui::disabled_button(btn_label(ui::glyph::TARGET, calibrate_label, scale), scale)
                    .width(Fill);
            tooltip(
                btn,
                container(
                    text("Start a test to calibrate your baseline")
                        .size(scale.text(13.0))
                        .color(T1),
                )
                .style(ui::tile_style)
                .padding(scale.pad(6.0, 10.0)),
                tooltip::Position::Top,
            )
            .gap(scale.px(6.0))
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
                        scale,
                    ),
                    ui::seg_button(
                        "Background",
                        app.session_start_mode == RunMode::Background,
                        Message::SessionModeSelected(RunMode::Background),
                        scale,
                    ),
                ]
                .spacing(scale.px(2.0)),
            )
            .padding(scale.pad_all(3.0))
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
                    ui::primary_button(btn_label(ui::glyph::PLAY, "Start Session", scale), scale)
                        .width(Fill)
                        .on_press(start_msg),
                ]
                .spacing(scale.px(10.0))
                .align_y(Alignment::Center),
            )
        } else {
            None
        };

        let mut col = column![ui::micro_label("Session", scale)]
            .spacing(scale.px(10.0))
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
        .padding(scale.pad_all(16.0))
        // Fixed height so the panel reserves room for the calibrate / start
        // buttons that appear mid-session, keeping the layout stable.
        .height(Length::Fixed(scale.px(PANEL_HEIGHT)))
        .width(Fill);

    // Settings gear hovers in the panel's top-right corner.
    let gear =
                container(tooltip(ui::icon_button(ui::glyph::GEAR, 20.0, scale).on_press(Message::OpenSettingsPressed),
                container(text("Settings").size(scale.text(15.0)).color(T1))
                    .style(ui::tile_style)
                    .padding(scale.pad(3.0, 10.0)),
                tooltip::Position::Bottom,
            )
            .gap(scale.px(2.0)))
        .align_x(Alignment::End)
        .align_y(Alignment::Start)
        .width(Fill)
        .height(Length::Fixed(scale.px(PANEL_HEIGHT)))
        .padding(scale.pad_all(5.0));


    stack![panel, gear].into()
}
