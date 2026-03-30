mod components;
mod subscriptions;
mod theme;

use std::time::Duration;

use crate::utils::ManagedService;
use iced::widget::{column, image, row};
use iced::{Element, Size, Subscription, Task, Theme, window};

const DEFAULT_POSTURE_THRESHOLD_DEG: f32 = 12.0;
const MAIN_WINDOW_SIZE: Size = Size::new(720.0, 420.0);
const DEBUG_WINDOW_SIZE: Size = Size::new(420.0, 200.0);

enum InferenceState {
    Unloaded,
    Stopped,
    Running,
}

pub fn run() -> iced::Result {
    iced::daemon(
        move || App::new(crate::new_pipelines()),
        App::update,
        App::view,
    )
    .title(App::title)
    .subscription(App::subscription)
    .theme(App::theme)
    .run()
}

pub struct App {
    pipelines: crate::Pipelines,
    main_window_id: window::Id,
    debug_window_id: Option<window::Id>,

    cam_frame: Option<image::Handle>,
    cv_frame: Option<image::Handle>,

    model_load_time: Option<Duration>,
    inference_time: Option<Duration>,
    posture_angle_deg: Option<f32>,
    posture_baseline_deg: Option<f32>,
    posture_threshold_deg: f32,
    bad_posture: bool,

    inference_state: InferenceState,
}

#[derive(Debug, Clone)]
pub enum Message {
    CamFrame(image::Handle),
    CvInference((image::Handle, Duration, Option<f32>)),
    WindowCloseRequested(window::Id),
    LoadModelPressed,
    StartInferencePressed,
    StopInferencePressed,
    PostureThresholdChanged(f32),
}

impl App {
    fn new(pipelines: crate::Pipelines) -> (Self, Task<Message>) {
        let (main_window_id, open_main_window) = window::open(Self::main_window_settings());
        let (debug_window_id, open_debug_window) = window::open(Self::debug_window_settings());

        (
            Self {
                pipelines,
                main_window_id,
                debug_window_id: Some(debug_window_id),
                cam_frame: None,
                cv_frame: None,
                model_load_time: None,
                inference_time: None,
                posture_angle_deg: None,
                posture_baseline_deg: None,
                posture_threshold_deg: DEFAULT_POSTURE_THRESHOLD_DEG,
                bad_posture: false,
                inference_state: InferenceState::Unloaded,
            },
            Task::batch([open_main_window.discard(), open_debug_window.discard()]),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CamFrame(frame) => {
                self.cam_frame = Some(frame);
                Task::none()
            }
            Message::CvInference((frame, inf_time, posture_angle_deg)) => {
                self.cv_frame = Some(frame);
                self.inference_time = Some(inf_time);
                self.posture_angle_deg = posture_angle_deg;

                if self.posture_baseline_deg.is_none() {
                    self.posture_baseline_deg = posture_angle_deg;
                }

                self.bad_posture = match (self.posture_baseline_deg, posture_angle_deg) {
                    (Some(baseline), Some(current)) => {
                        (current - baseline).abs() >= self.posture_threshold_deg
                    }
                    _ => false,
                };
                Task::none()
            }
            Message::WindowCloseRequested(window_id) => {
                if window_id == self.main_window_id {
                    iced::exit()
                } else if self.debug_window_id == Some(window_id) {
                    self.debug_window_id = None;
                    window::close(window_id)
                } else {
                    Task::none()
                }
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
                Task::none()
            }
            Message::StartInferencePressed => {
                self.pipelines
                    .camera_manager
                    .start()
                    .expect("Unable to start camera");
                self.pipelines
                    .cv_manager
                    .start()
                    .expect("Unable to start model");
                self.posture_angle_deg = None;
                self.posture_baseline_deg = None;
                self.bad_posture = false;
                self.inference_state = InferenceState::Running;
                Task::none()
            }
            Message::StopInferencePressed => {
                self.pipelines.camera_manager.stop();
                self.pipelines.cv_manager.stop();
                self.bad_posture = false;
                self.inference_state = InferenceState::Stopped;
                Task::none()
            }
            Message::PostureThresholdChanged(threshold_deg) => {
                self.posture_threshold_deg = threshold_deg;
                self.bad_posture = match (self.posture_baseline_deg, self.posture_angle_deg) {
                    (Some(baseline), Some(current)) => {
                        (current - baseline).abs() >= self.posture_threshold_deg
                    }
                    _ => false,
                };
                Task::none()
            }
        }
    }

    fn view(&self, window_id: window::Id) -> Element<'_, Message> {
        if window_id == self.main_window_id {
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
        } else if self.debug_window_id == Some(window_id) {
            components::debug_stats::view(self)
        } else {
            iced::widget::text("Unknown window").into()
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            subscriptions::raw_frame_subscription(self.pipelines.camera_manager.clone())
                .map(Message::CamFrame),
            subscriptions::inference_subscription(self.pipelines.cv_manager.clone())
                .map(Message::CvInference),
            window::close_requests().map(Message::WindowCloseRequested),
        ])
    }

    fn theme(&self, _window_id: window::Id) -> Theme {
        theme::custom_theme()
    }

    fn title(&self, window_id: window::Id) -> String {
        if self.debug_window_id == Some(window_id) {
            "Debug Stats".to_string()
        } else {
            "PostureTracker".to_string()
        }
    }

    fn main_window_settings() -> window::Settings {
        window::Settings {
            size: MAIN_WINDOW_SIZE,
            resizable: false,
            minimizable: false,
            level: window::Level::AlwaysOnTop,
            position: window::Position::Centered,
            exit_on_close_request: false,
            ..Default::default()
        }
    }

    fn debug_window_settings() -> window::Settings {
        window::Settings {
            size: DEBUG_WINDOW_SIZE,
            resizable: false,
            position: window::Position::Centered,
            exit_on_close_request: false,
            ..Default::default()
        }
    }
}
