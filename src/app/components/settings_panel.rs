use iced::widget::{button, column, container, pick_list, row, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::app::theme::{AMBER, ELEV, GREEN, PANEL, T1};
use crate::app::{App, Message, MIN_ALERT_COOLDOWN_SECS};

const FIELD_WIDTH: f32 = 300.0;

fn flat_button(label: &str) -> button::Button<'_, Message> {
    button(text(label).size(14))
        .padding([6, 14])
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => ELEV,
                _ => PANEL,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: T1,
                border: Border::default().rounded(6),
                ..button::Style::default()
            }
        })
}

/// Full-page Settings view shown in place of the dashboard.
pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        text("Settings").size(24),
        Space::new().width(Length::Fill),
        flat_button("Back").on_press(Message::CloseSettingsPressed),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let content = column![
        header,
        text("Camera").size(18),
        camera_selector(app),
        Space::new().height(Length::Fixed(8.0)),
        text("Posture Alerts").size(18),
        cooldown_field(app),
        Space::new().height(Length::Fixed(8.0)),
        text("Window").size(18),
        window_actions(app),
    ]
    .spacing(14)
    .padding(24)
    .width(Length::Fill)
    .height(Length::Fill);

    container(content)
        .style(|_| container::Style {
            background: Some(Background::Color(PANEL)),
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// The camera dropdown (or a "no cameras" notice) at the given width.
fn camera_pick_list(app: &App, width: Length) -> Element<'_, Message> {
    if app.available_cameras.is_empty() {
        return text("No cameras found").size(14).into();
    }

    let selected = app
        .available_cameras
        .iter()
        .find(|o| app.config.camera.device.as_deref() == Some(o.path.as_str()))
        .cloned();

    pick_list(
        app.available_cameras.clone(),
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
        camera_pick_list(app, Length::Fixed(FIELD_WIDTH)),
        flat_button("Refresh").on_press(Message::RefreshCamerasPressed),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

/// Buttons relocated from the old control-panel dropdown.
fn window_actions(app: &App) -> Element<'_, Message> {
    let mut col = column![].spacing(8);
    if !app.has_debug_window() {
        col = col.push(flat_button("Debug Window").on_press(Message::OpenDebugWindowPressed));
    }
    col = col.push(flat_button("Hide To Tray / Minimize").on_press(Message::HideMainWindowPressed));
    col = col.push(flat_button("Quit App").on_press(Message::QuitRequested));
    col.into()
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
        text("Minimum time before the alert can reappear after it's dismissed").size(13),
        input,
    ]
    .spacing(8);

    if let Some(warning) = cooldown_warning(&app.cooldown_input) {
        col = col.push(text(warning).size(13).color(AMBER).width(Length::Fixed(FIELD_WIDTH)));
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
    let confirm = {
        let btn = flat_button("Confirm").width(Length::Fill);
        if app.config.camera.device.is_some() {
            btn.on_press(Message::CameraPromptConfirmed)
        } else {
            btn
        }
    };

    let card = container(
        column![
            text("Choose a camera").size(16),
            camera_pick_list(app, Length::Fill),
            confirm,
        ]
        .spacing(14),
    )
    .width(Length::Fixed(280.0))
    .padding(20)
    .style(|_| container::Style {
        background: Some(Background::Color(PANEL)),
        border: Border {
            color: Color { a: 0.5, ..GREEN },
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    });

    // Dim the background and center the dialog.
    container(card)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(Color { a: 0.7, ..Color::BLACK })),
            ..Default::default()
        })
        .into()
}
