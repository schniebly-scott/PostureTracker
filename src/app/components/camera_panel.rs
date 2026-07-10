use iced::border::Border;
use iced::widget::{column, container, image, row, stack, text, Space};
use iced::{Alignment, Background, Color, ContentFit, Element, Length, Length::Fill};

use crate::app::components::ui;
use crate::app::components::ui::Scale;
use crate::app::theme::{ELEV, GREEN, LINE, RED, SCRIM, T2, T3, VIDEO_BG};
use crate::app::{App, Message};

/// Purposeful empty state: icon, copy, and a calibration tip — not a lone box.
fn idle_card<'a>(scale: Scale) -> Element<'a, Message> {
    let box_side = scale.px(56.0);
    let icon = container(ui::icon(ui::glyph::CAMERA, scale.px(26.0)).color(T3))
        .width(box_side)
        .height(box_side)
        .center_x(box_side)
        .center_y(box_side)
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
            ui::icon(ui::glyph::TARGET, scale.px(16.0)).color(T3),
            text("Sit upright before calibrating")
                .size(scale.text(15.0))
                .color(T3),
        ]
        .spacing(scale.px(6.0))
        .align_y(Alignment::Center),
    )
    .padding(scale.pad(6.0, 11.0))
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
            text("Camera idle")
                .size(scale.text(20.0))
                .font(ui::semibold())
                .color(T2),
            container(
                text("Start a session to begin tracking your head and shoulder alignment.")
                    .size(scale.text(15.0))
                    .color(T3)
                    .align_x(Alignment::Center),
            )
            .max_width(scale.px(240.0)),
            tip,
        ]
        .spacing(scale.px(14.0))
        .align_x(Alignment::Center),
    )
    .center_x(Fill)
    .center_y(Fill)
    .into()
}

/// A floating chip showing the live head-to-shoulder-angle deviation from baseline.
fn angle_chip<'a>(app: &App, scale: Scale) -> Element<'a, Message> {
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
            text("HEAD-TO-SHOULDER ANGLE")
                .size(scale.text(10.0))
                .font(ui::semibold())
                .color(T3),
            ui::value(label, scale.text(18.0), color).font(ui::mono()),
        ]
        .spacing(scale.px(9.0))
        .align_y(Alignment::Center),
    )
    .padding(scale.pad(8.0, 12.0))
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
fn rec_chip<'a>(app: &App, scale: Scale) -> Element<'a, Message> {
    let label = if app.is_background_mode() {
        "TRACKING"
    } else {
        "TESTING"
    };
    let dot_color = if app.bad_posture { RED } else { GREEN };

    let dot = container(Space::new().width(scale.px(8.0)).height(scale.px(8.0))).style(
        move |_| container::Style {
            background: Some(Background::Color(dot_color)),
            border: Border::default().rounded(4),
            ..Default::default()
        },
    );

    container(
        row![
            dot,
            text(label)
                .size(scale.text(13.0))
                .font(ui::semibold())
                .color(T2)
        ]
        .spacing(scale.px(7.0))
        .align_y(Alignment::Center),
    )
    .padding(scale.pad(7.0, 11.0))
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

pub fn view(app: &App, scale: Scale) -> Element<'_, Message> {
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
                    angle_chip(app, scale),
                    Space::new().width(Fill),
                    rec_chip(app, scale),
                ]
                .width(Fill),
            )
            .padding(scale.pad_all(14.0))
            .width(Fill)
            .height(Fill)
            .align_y(Alignment::Start);

            stack![feed, overlay].into()
        }
        None => idle_card(scale),
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
        // Takes the larger share of the dashboard row; the controls/metrics
        // column to its right is FillPortion(2).
        .width(Length::FillPortion(3))
        .height(Fill)
        .into()
}
