use iced::widget::{column, container, pick_list, row, text, text_input, Space};
use iced::{Alignment, Background, Color, Element, Length};
use iced::Length::Fill;

use crate::app::components::ui;
use crate::app::components::ui::Scale;
use crate::app::theme::{AMBER, T2};
use crate::app::{App, Message, MIN_ALERT_COOLDOWN_SECS};
use crate::camera::CaptureResolution;

/// A semibold text label sized for the kit button system.
fn btn_label(label: &str, scale: Scale) -> Element<'_, Message> {
    text(label).size(scale.text(14.0)).font(ui::semibold()).into()
}

/// Wrap a section's body in a `panel_style` card led by a `micro_label`
/// header — the same rhythm as the dashboard columns.
fn section<'a>(
    title: &'static str,
    body: Element<'a, Message>,
    scale: Scale,
) -> Element<'a, Message> {
    container(column![ui::micro_label(title, scale), body].spacing(scale.px(12.0)))
        .style(ui::panel_style)
        .padding(scale.pad_all(16.0))
        .width(Fill)
        .into()
}

/// Full-page Settings view shown in place of the dashboard.
pub fn view(app: &App, scale: Scale) -> Element<'_, Message> {
    let header = row![
        text("Settings").size(scale.text(24.0)),
        Space::new().width(Fill),
        ui::secondary_button(btn_label("Back", scale), scale)
            .on_press(Message::CloseSettingsPressed),
    ]
    .align_y(Alignment::Center)
    .width(Fill);

    let content = column![
        header,
        section("Camera", camera_selector(app, scale), scale),
        section("Posture Alerts", cooldown_field(app, scale), scale),
        section("Window", window_actions(app, scale), scale),
    ]
    .spacing(scale.px(14.0))
    .padding(scale.pad_all(24.0))
    .width(Fill)
    .height(Fill);

    // No explicit background — the theme backdrop (BG) shows through, matching
    // the dashboard's cards-on-backdrop look.
    container(content).width(Fill).height(Fill).into()
}

/// The camera dropdown (or a "no cameras" notice) at the given width.
fn camera_pick_list(app: &App, width: Length, scale: Scale) -> Element<'_, Message> {
    if app.available_cameras.is_empty() {
        return text("No cameras found").size(scale.text(14.0)).color(T2).into();
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
    .text_size(scale.text(14.0))
    .width(width)
    .padding(scale.pad(8.0, 12.0))
    .into()
}

/// Settings-page camera row: device dropdown plus a refresh button, followed by
/// the capture-resolution cap.
fn camera_selector(app: &App, scale: Scale) -> Element<'_, Message> {
    let device_row = row![
        camera_pick_list(app, Fill, scale),
        ui::secondary_button(btn_label("Refresh", scale), scale)
            .on_press(Message::RefreshCamerasPressed),
    ]
    .spacing(scale.px(10.0))
    .align_y(Alignment::Center);

    let resolution = column![
        text("Resolution").size(scale.text(13.0)).color(T2),
        resolution_pick_list(app, scale),
        text(
            "Caps the processed and displayed frame size (aspect ratio is preserved). \
             Smaller sizes use less CPU/GPU and can smooth out a stuttering feed."
        )
        .size(scale.text(12.0))
        .color(T2),
    ]
    .spacing(scale.px(6.0));

    column![device_row, resolution].spacing(scale.px(14.0)).into()
}

/// The capture-resolution cap dropdown. The stored width/height always match one
/// of `CaptureResolution::OPTIONS` (the user can only pick from this list), so a
/// selection is highlighted rather than falling back to the placeholder.
fn resolution_pick_list(app: &App, scale: Scale) -> Element<'_, Message> {
    let selected = CaptureResolution::OPTIONS.iter().copied().find(|r| {
        r.width == app.config.camera.capture_width && r.height == app.config.camera.capture_height
    });

    pick_list(
        &CaptureResolution::OPTIONS[..],
        selected,
        Message::CaptureResolutionSelected,
    )
    .placeholder("Select a resolution")
    .text_size(scale.text(14.0))
    .width(Fill)
    .padding(scale.pad(8.0, 12.0))
    .into()
}

/// Buttons relocated from the old control-panel dropdown.
fn window_actions(app: &App, scale: Scale) -> Element<'_, Message> {
    // Keep the row present but disabled while the debug window is open, so the
    // buttons below it don't shift.
    let debug_btn = if app.has_debug_window() {
        ui::disabled_button(btn_label("Debug Window", scale), scale).width(Fill)
    } else {
        ui::secondary_button(btn_label("Debug Window", scale), scale)
            .width(Fill)
            .on_press(Message::OpenDebugWindowPressed)
    };

    column![
        debug_btn,
        ui::secondary_button(btn_label("Hide To Tray / Minimize", scale), scale)
            .width(Fill)
            .on_press(Message::HideMainWindowPressed),
        ui::danger_button(btn_label("Quit App", scale), scale)
            .width(Fill)
            .on_press(Message::QuitRequested),
    ]
    .spacing(scale.px(8.0))
    .into()
}

/// Settings-page control for the alert cooldown — the minimum time before the
/// bad-posture popup can reappear after it's dismissed.
fn cooldown_field(app: &App, scale: Scale) -> Element<'_, Message> {
    let input = row![
        text_input("5", &app.cooldown_input)
            .on_input(Message::CooldownInputChanged)
            .on_submit(Message::CommitConfig)
            .size(scale.text(14.0))
            .width(Length::Fixed(scale.px(80.0)))
            .padding(scale.pad(8.0, 12.0)),
        text("seconds").size(scale.text(14.0)),
    ]
    .spacing(scale.px(10.0))
    .align_y(Alignment::Center);

    let mut col = column![
        text("Minimum time before the alert can reappear after it's dismissed")
            .size(scale.text(13.0))
            .color(T2),
        input,
    ]
    .spacing(scale.px(8.0));

    if let Some(warning) = cooldown_warning(&app.cooldown_input) {
        col = col.push(text(warning).size(scale.text(13.0)).color(AMBER).width(Fill));
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
/// Renders unscaled: its small centered card fits even the minimum window.
pub fn camera_prompt(app: &App) -> Element<'_, Message> {
    // The page's one CTA — primary when a camera is picked, visibly disabled
    // until then.
    let confirm = if app.config.camera.device.is_some() {
        ui::primary_button(btn_label("Confirm", Scale::ONE), Scale::ONE)
            .width(Fill)
            .on_press(Message::CameraPromptConfirmed)
    } else {
        ui::disabled_button(btn_label("Confirm", Scale::ONE), Scale::ONE).width(Fill)
    };

    let card = container(
        column![
            text("Choose a camera").size(16).font(ui::semibold()),
            camera_pick_list(app, Fill, Scale::ONE),
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
