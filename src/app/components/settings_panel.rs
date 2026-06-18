use iced::widget::{column, container, pick_list, row, text, text_input, Space};
use iced::{Alignment, Background, Color, Element, Length};
use iced::Length::Fill;

use crate::app::components::ui;
use crate::app::theme::{AMBER, T2};
use crate::app::{App, Message, MIN_ALERT_COOLDOWN_SECS};

/// A semibold text label sized for the kit button system.
fn btn_label(label: &str) -> Element<'_, Message> {
    text(label).size(14).font(ui::semibold()).into()
}

/// Wrap a section's body in a `panel_style` card led by a `micro_label`
/// header — the same rhythm as the dashboard columns.
fn section<'a>(title: &'static str, body: Element<'a, Message>) -> Element<'a, Message> {
    container(column![ui::micro_label(title), body].spacing(12))
        .style(ui::panel_style)
        .padding(16)
        .width(Fill)
        .into()
}

/// Full-page Settings view shown in place of the dashboard.
pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        text("Settings").size(24),
        Space::new().width(Fill),
        ui::secondary_button(btn_label("Back")).on_press(Message::CloseSettingsPressed),
    ]
    .align_y(Alignment::Center)
    .width(Fill);

    let content = column![
        header,
        section("Camera", camera_selector(app)),
        section("Posture Alerts", cooldown_field(app)),
        section("Window", window_actions(app)),
    ]
    .spacing(14)
    .padding(24)
    .width(Fill)
    .height(Fill);

    // No explicit background — the theme backdrop (BG) shows through, matching
    // the dashboard's cards-on-backdrop look.
    container(content).width(Fill).height(Fill).into()
}

/// The camera dropdown (or a "no cameras" notice) at the given width.
fn camera_pick_list(app: &App, width: Length) -> Element<'_, Message> {
    if app.available_cameras.is_empty() {
        return text("No cameras found").size(14).color(T2).into();
    }

    let selected = app
        .available_cameras
        .iter()
        .find(|o| app.config.camera.device.as_deref() == Some(o.path.as_str()))
        .cloned();

    pick_list(
        app.available_cameras.as_slice(),
        selected,
        Message::CameraSelected,
    )
    .placeholder("Select a camera")
    .text_size(14)
    .width(width)
    .padding([8, 12])
    .into()
}

/// Settings-page camera row: dropdown plus a refresh button.
fn camera_selector(app: &App) -> Element<'_, Message> {
    row![
        camera_pick_list(app, Fill),
        ui::secondary_button(btn_label("Refresh")).on_press(Message::RefreshCamerasPressed),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

/// Buttons relocated from the old control-panel dropdown.
fn window_actions(app: &App) -> Element<'_, Message> {
    // Keep the row present but disabled while the debug window is open, so the
    // buttons below it don't shift.
    let debug_btn = if app.has_debug_window() {
        ui::disabled_button(btn_label("Debug Window")).width(Fill)
    } else {
        ui::secondary_button(btn_label("Debug Window"))
            .width(Fill)
            .on_press(Message::OpenDebugWindowPressed)
    };

    column![
        debug_btn,
        ui::secondary_button(btn_label("Hide To Tray / Minimize"))
            .width(Fill)
            .on_press(Message::HideMainWindowPressed),
        ui::danger_button(btn_label("Quit App"))
            .width(Fill)
            .on_press(Message::QuitRequested),
    ]
    .spacing(8)
    .into()
}

/// Settings-page control for the alert cooldown — the minimum time before the
/// bad-posture popup can reappear after it's dismissed.
fn cooldown_field(app: &App) -> Element<'_, Message> {
    let input = row![
        text_input("60", &app.cooldown_input)
            .on_input(Message::CooldownInputChanged)
            .size(14)
            .width(Length::Fixed(80.0))
            .padding([8, 12]),
        text("seconds").size(14),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let mut col = column![
        text("Minimum time before the alert can reappear after it's dismissed")
            .size(13)
            .color(T2),
        input,
    ]
    .spacing(8);

    if let Some(warning) = cooldown_warning(&app.cooldown_input) {
        col = col.push(text(warning).size(13).color(AMBER).width(Fill));
    }

    col.into()
}

/// Warning text for the cooldown field, or `None` when the input is a valid
/// value at/above the floor.
fn cooldown_warning(input: &str) -> Option<String> {
    match input.trim().parse::<u64>() {
        Ok(secs) if secs >= MIN_ALERT_COOLDOWN_SECS => None,
        Ok(_) => Some(format!(
            "Cooldown must be at least {MIN_ALERT_COOLDOWN_SECS} seconds — shorter \
             values make the alert reappear before you've had time to correct your \
             posture, so it can pop up repeatedly."
        )),
        Err(_) => Some("Enter a whole number of seconds.".to_string()),
    }
}

/// Modal overlay shown at first run when no camera has been configured yet.
pub fn camera_prompt(app: &App) -> Element<'_, Message> {
    // The page's one CTA — primary when a camera is picked, visibly disabled
    // until then.
    let confirm = if app.config.camera.device.is_some() {
        ui::primary_button(btn_label("Confirm"))
            .width(Fill)
            .on_press(Message::CameraPromptConfirmed)
    } else {
        ui::disabled_button(btn_label("Confirm")).width(Fill)
    };

    let card = container(
        column![
            text("Choose a camera").size(16).font(ui::semibold()),
            camera_pick_list(app, Fill),
            confirm,
        ]
        .spacing(14),
    )
    .width(Length::Fixed(320.0))
    .padding(20)
    .style(ui::panel_style);

    // Dim the background and center the dialog.
    container(card)
        .center_x(Fill)
        .center_y(Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(Color { a: 0.7, ..Color::BLACK })),
            ..Default::default()
        })
        .into()
}
