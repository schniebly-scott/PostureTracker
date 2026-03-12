mod subscriptions;
mod theme;
mod components;

use std::time::Duration;

use iced::advanced::graphics::core::window;
use iced::widget::{column, row, image};
use iced::{Element, Size, Subscription, Theme};
use crate::{Frame, Inference};
use crate::utils::ManagedService;

enum InferenceState {
    Unloaded,
    Stopped,
    Running,
}

pub fn run() -> iced::Result {
    iced::application(
            move || App::new(crate::new_pipelines()),
            App::update,
            App::view,
        )
        .subscription(App::subscription)
        .theme(App::theme)
        .window(window::Settings {
            size: Size::new(720.0, 480.0),
            resizable: false,
            minimizable: false,
            level: window::Level::AlwaysOnTop,
            position: window::Position::Centered,
            ..Default::default()
        })
        .run()
}

pub struct App {
    pipelines: crate::Pipelines,

    cam_frame: Option<image::Handle>,
    cv_frame: Option<image::Handle>,
    
    model_load_time: Option<Duration>,
    inference_time: Option<Duration>,

    inference_state: InferenceState,
}

#[derive(Debug, Clone)]
pub enum Message {
    CamFrame(image::Handle),
    CvInference((image::Handle, Duration)),
    LoadModelPressed,
    StartInferencePressed,
    StopInferencePressed,
}

impl App {
    fn new(pipelines: crate::Pipelines) -> Self {
        Self {
            pipelines,
            cam_frame: None,
            cv_frame: None,
            model_load_time: None,
            inference_time: None,
            inference_state: InferenceState::Unloaded,
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::CamFrame(frame) => {
                self.cam_frame = Some(frame);
            }
            Message::CvInference((frame, inf_time)) => {
                self.cv_frame = Some(frame);
                self.inference_time = Some(inf_time);
            }
            Message::LoadModelPressed => {
                match self.pipelines.cv_manager.load_model() {
                    Ok(elapsed) => {
                        self.model_load_time = Some(elapsed);
                    }
                    Err(e) => {
                        eprintln!("Unable to load model: {}", e)
                    }
                };
                self.inference_state = InferenceState::Stopped;
            }
            Message::StartInferencePressed => {
                self.pipelines.camera_manager.start().expect("Unable to start camera");
                self.pipelines.cv_manager.start().expect("Unable to start model");
                self.inference_state = InferenceState::Running;
            }
            Message::StopInferencePressed => {
                self.pipelines.camera_manager.stop();
                self.pipelines.cv_manager.stop();
                self.inference_state = InferenceState::Stopped;
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        column![
            row![
                components::camera_panel::view(self),
                column![
                    components::control_panel::view(self),
                    components::metrics_panel::view(self),
                ]
                .width(iced::Length::FillPortion(1))
            ],
            components::status_panel::view(self)
        ]
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            subscriptions::raw_frame_subscription(self.pipelines.camera_manager.clone()).map(Message::CamFrame),
            subscriptions::inference_subscription(self.pipelines.cv_manager.clone()).map(Message::CvInference),
        ])
    }

    fn theme(&self) -> Theme {
        theme::custom_theme()
    }
}
