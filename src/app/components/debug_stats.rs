use std::collections::VecDeque;

use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::{column, container, row, text};
use iced::{Background, Color, Element, Length, Point, Rectangle, Renderer, Theme};
use iced::mouse;

use crate::app::theme::{DARK_BLUE, LIGHT_BLUE, OWHITE, WARNING_RED};
use crate::app::{App, Message};
use crate::metrics::AngleSample;

const HISTORY_SECS: f64 = 120.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let current_angle = app
        .posture_angle_deg
        .map(|a| format!("{a:.1} deg"))
        .unwrap_or_else(|| "--".to_string());

    let baseline_angle = app
        .posture_baseline_deg
        .map(|a| format!("{a:.1} deg"))
        .unwrap_or_else(|| "--".to_string());

    let delta_angle = match (app.posture_baseline_deg, app.posture_angle_deg) {
        (Some(b), Some(a)) => format!("{:.1} deg", (a - b).abs()),
        _ => "--".to_string(),
    };

    let stats = column![
        row![text("Current Angle:"),  text(current_angle)].spacing(10),
        row![text("Baseline Angle:"), text(baseline_angle)].spacing(10),
        row![text("Angle Delta:"),    text(delta_angle)].spacing(10),
    ]
    .spacing(10);

    let chart = canvas::Canvas::new(AngleChart {
        history: &app.metrics.angle_history,
        baseline: app.posture_baseline_deg,
        threshold: app.posture_threshold_deg,
    })
    .width(Length::Fill)
    .height(180);

    container(
        column![
            text("Debug Stats").size(20),
            stats,
            text("Angle History (2 min)").size(14),
            chart,
        ]
        .spacing(10),
    )
    .style(move |_| container::Style {
        background: Some(Background::Color(if app.bad_posture {
            WARNING_RED
        } else {
            DARK_BLUE
        })),
        ..Default::default()
    })
    .padding(15)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

struct AngleChart<'a> {
    history: &'a VecDeque<AngleSample>,
    baseline: Option<f32>,
    threshold: f32,
}

impl<Message> canvas::Program<Message> for AngleChart<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let angles: Vec<f32> = self
            .history
            .iter()
            .filter_map(|s| s.angle_deg)
            .collect();

        if angles.is_empty() {
            return vec![];
        }

        let mut frame = Frame::new(renderer, bounds.size());

        // Determine Y range — include baseline ± threshold with padding
        let data_min = angles.iter().cloned().fold(f32::INFINITY, f32::min);
        let data_max = angles.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        let (y_min, y_max) = if let Some(b) = self.baseline {
            let lo = data_min.min(b - self.threshold - 5.0);
            let hi = data_max.max(b + self.threshold + 5.0);
            (lo, hi)
        } else {
            (data_min - 5.0, data_max + 5.0)
        };

        let y_range = (y_max - y_min).max(1.0);

        let to_x = |sample: &AngleSample| -> f32 {
            let elapsed = sample.captured_at.elapsed().as_secs_f64();
            let t = (1.0 - elapsed / HISTORY_SECS).max(0.0);
            bounds.width * t as f32
        };

        let to_y = |angle: f32| -> f32 {
            bounds.height * (1.0 - (angle - y_min) / y_range)
        };

        // Baseline and threshold reference lines
        if let Some(b) = self.baseline {
            let baseline_y = to_y(b);
            let baseline_path = Path::new(|p| {
                p.move_to(Point::new(0.0, baseline_y));
                p.line_to(Point::new(bounds.width, baseline_y));
            });
            frame.stroke(
                &baseline_path,
                Stroke::default().with_color(OWHITE).with_width(1.0),
            );

            for &offset in &[self.threshold, -self.threshold] {
                let ty = to_y(b + offset);
                let t_path = Path::new(|p| {
                    p.move_to(Point::new(0.0, ty));
                    p.line_to(Point::new(bounds.width, ty));
                });
                frame.stroke(
                    &t_path,
                    Stroke::default()
                        .with_color(Color { a: 0.6, ..WARNING_RED })
                        .with_width(1.0),
                );
            }
        }

        // Angle line
        let line = Path::new(|p| {
            let mut started = false;
            for sample in self.history.iter() {
                let Some(angle) = sample.angle_deg else {
                    continue;
                };
                let pt = Point::new(to_x(sample), to_y(angle));
                if started {
                    p.line_to(pt);
                } else {
                    p.move_to(pt);
                    started = true;
                }
            }
        });
        frame.stroke(
            &line,
            Stroke::default().with_color(LIGHT_BLUE).with_width(2.0),
        );

        vec![frame.into_geometry()]
    }
}
