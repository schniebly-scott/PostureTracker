use iced::widget::{column, container, mouse_area, responsive, text};
use iced::{Background, Element, Length::Fill};

use crate::app::theme::RED;
use crate::app::{App, Message};

pub fn view(_app: &App) -> Element<'_, Message> {
    // The alert may be shown full-screen on anything from a small laptop to a
    // 4K panel, or — if a window manager declines the fullscreen request — at
    // the 1000×600 baseline. Derive the type sizes from the actual width so the
    // headline reads well everywhere instead of dominating a small surface or
    // looking lost on a large one.
    let overlay = responsive(|size| {
        let (headline_size, caption_size) = scale_for_width(size.width);

        let content = column![
            text("BAD POSTURE DETECTED").size(headline_size),
            text("Correct your posture, then press Esc or click to dismiss").size(caption_size),
        ]
        .align_x(iced::Alignment::Center)
        .spacing(30);

        container(content)
            .center_x(Fill)
            .center_y(Fill)
            .width(Fill)
            .height(Fill)
            .into()
    });

    mouse_area(
        container(overlay)
            .style(|_| container::Style {
                background: Some(Background::Color(RED)),
                ..Default::default()
            })
            .width(Fill)
            .height(Fill),
    )
    .on_press(Message::DismissAlert)
    .into()
}

/// (headline, caption) text sizes in logical pixels, bucketed by the overlay
/// width. Tuned so the headline fills the field of red without overflowing the
/// 1000×600 baseline, and keeps presence on large/4K surfaces.
fn scale_for_width(width: f32) -> (f32, f32) {
    if width < 1280.0 {
        (44.0, 20.0)
    } else if width < 2000.0 {
        (72.0, 28.0)
    } else {
        (104.0, 38.0)
    }
}
