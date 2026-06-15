use iced::border::Border;
use iced::widget::{column, container, image, row, stack, text, Space};
use iced::{Alignment, Background, Color, ContentFit, Element, Length::Fill};

use crate::app::components::ui;
use crate::app::theme::{ELEV, GREEN, LINE, RED, SCRIM, T2, T3, VIDEO_BG};
use crate::app::{App, Message};

/// Purposeful empty state: icon, copy, and a calibration tip — not a lone box.
fn idle_card<'a>() -> Element<'a, Message> {
    let icon = container(text("\u{25A3}").size(26).color(T3))
        .width(56)
        .height(56)
        .center_x(56)
        .center_y(56)
        .style(|_| container::Style {
            background: Some(Background::Color(ELEV)),
            border: Border {
                color: LINE,
                width: 1.0,
                radius: 16.0.into(),
            },
            ..Default::default()
        });

    let tip = container(
        row![
            text("\u{25CE}").size(16).color(T3),
            text("Sit upright before calibrating").size(15).color(T3),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([6, 11])
    .style(|_| container::Style {
        background: Some(Background::Color(ELEV)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 999.0.into(),
        },
        ..Default::default()
    });

    container(
        column![
            icon,
            text("Camera idle").size(20).font(ui::semibold()).color(T2),
            container(
                text("Start a session to begin tracking your head and shoulder alignment.")
                    .size(15)
                    .color(T3)
                    .align_x(Alignment::Center),
            )
            .max_width(240),
            tip,
        ]
        .spacing(14)
        .align_x(Alignment::Center),
    )
    .center_x(Fill)
    .center_y(Fill)
    .into()
}

/// A floating chip showing the live head-to-shoulder-angle deviation from baseline.
fn angle_chip<'a>(app: &App) -> Element<'a, Message> {
    let (label, color) = match (app.posture_baseline_deg, app.posture_angle_deg) {
        (Some(b), Some(a)) => (
            format!("{:.1}\u{00B0}", (a - b).abs()),
            if app.bad_posture { RED } else { GREEN },
        ),
        (None, Some(a)) => (format!("{a:.1}\u{00B0}"), T2),
        _ => ("\u{2014}".to_string(), T3),
    };

    container(
        row![
            text("HEAD-TO-SHOULDER ANGLE").size(10).font(ui::semibold()).color(T3),
            ui::value(label, 18, color).font(ui::mono()),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .padding([8, 12])
    .style(|_| container::Style {
        background: Some(Background::Color(SCRIM)),
        border: Border {
            color: ui::with_alpha(Color::WHITE, 0.10),
            width: 1.0,
            radius: 11.0.into(),
        },
        ..Default::default()
    })
    .into()
}

/// "TRACKING" / "TESTING" pill in the top-right of an active feed.
fn rec_chip<'a>(app: &App) -> Element<'a, Message> {
    let label = if app.is_background_mode() {
        "TRACKING"
    } else {
        "TESTING"
    };
    let dot_color = if app.bad_posture { RED } else { GREEN };

    let dot = container(Space::new().width(8).height(8)).style(move |_| container::Style {
        background: Some(Background::Color(dot_color)),
        border: Border::default().rounded(4),
        ..Default::default()
    });

    container(
        row![dot, text(label).size(13).font(ui::semibold()).color(T2)]
            .spacing(7)
            .align_y(Alignment::Center),
    )
    .padding([7, 11])
    .style(|_| container::Style {
        background: Some(Background::Color(SCRIM)),
        border: Border {
            color: ui::with_alpha(Color::WHITE, 0.09),
            width: 1.0,
            radius: 999.0.into(),
        },
        ..Default::default()
    })
    .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let video: Option<Element<_>> = match (&app.cam_frame, &app.cv_frame) {
        (Some(cam), Some(cv)) => Some(
            stack![
                image(cam.clone())
                    .content_fit(ContentFit::Contain)
                    .width(Fill)
                    .height(Fill),
                image(cv.clone())
                    .content_fit(ContentFit::Contain)
                    .width(Fill)
                    .height(Fill),
            ]
            .into(),
        ),
        (Some(cam), None) => Some(
            image(cam.clone())
                .content_fit(ContentFit::Contain)
                .width(Fill)
                .height(Fill)
                .into(),
        ),
        _ => None,
    };

    let inner: Element<_> = match video {
        Some(feed) => {
            // Floating chips: angle (top-left) and tracking state (top-right).
            let overlay = container(
                row![
                    angle_chip(app),
                    Space::new().width(Fill),
                    rec_chip(app),
                ]
                .width(Fill),
            )
            .padding(14)
            .width(Fill)
            .height(Fill)
            .align_y(Alignment::Start);

            stack![feed, overlay].into()
        }
        None => idle_card(),
    };

    // "Clear but firm": the feed border picks up red when posture is bad.
    let border_color = if app.bad_posture {
        ui::mix(LINE, RED, 0.55)
    } else {
        LINE
    };

    container(inner)
        .style(move |_| container::Style {
            background: Some(Background::Color(VIDEO_BG)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 14.0.into(),
            },
            ..Default::default()
        })
        .clip(true)
        .width(Fill)
        .height(Fill)
        .into()
}
