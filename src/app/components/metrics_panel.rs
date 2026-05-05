use std::time::Duration;

use iced::font::Weight;
use iced::widget::{column, container, row, text};
use iced::{Background, Element, Font};

use crate::app::theme::MID_BLUE;
use crate::app::{App, Message};

fn fmt_duration(d: Option<Duration>) -> String {
    let Some(d) = d else {
        return "--".to_string();
    };
    let secs = d.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn metric_row<'a>(label: &'a str, value: impl ToString + 'a) -> Element<'a, Message> {
    row![
        text(label).font(Font { weight: Weight::Bold, ..Font::default() }),
        text(value.to_string()),
    ]
    .spacing(8)
    .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let m = &app.metrics;

    container(
        column![
            text("Metrics").size(20),
            metric_row("Breaks in posture:",   m.breaks_today().to_string()),
            metric_row("Time since last break:", fmt_duration(m.time_since_last_break())),
            metric_row("Good posture streak:",  fmt_duration(m.good_posture_streak())),
            metric_row("Bad posture today:",    fmt_duration(Some(m.bad_posture_duration_today()))),
            metric_row("Tracked today:",        fmt_duration(Some(m.tracked_duration_today()))),
        ]
        .spacing(6),
    )
    .style(|_| container::Style {
        background: Some(Background::Color(MID_BLUE)),
        ..Default::default()
    })
    .padding(15)
    .width(iced::Length::Fill)
    .height(iced::Length::Fill)
    .into()
}
