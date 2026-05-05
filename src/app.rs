mod components;
mod subscriptions;
mod theme;
mod tray;

use std::time::{Duration, Instant};

use crate::cv::TimeMetrics;
use crate::metrics::MetricsStore;
use crate::utils::ManagedService;
use iced::widget::{column, image, row};
use iced::{Element, Size, Subscription, Task, Theme, window};

const DEFAULT_POSTURE_THRESHOLD_DEG: f32 = 12.0;
const BACKGROUND_SAMPLE_COUNT: usize = 3;
const ALERT_COOLDOWN: Duration = Duration::from_secs(60);
const MAIN_WINDOW_SIZE: Size = Size::new(1000.0, 650.0);
const DEBUG_WINDOW_SIZE: Size = Size::new(720.0, 420.0);
const ALERT_WINDOW_SIZE: Size = Size::new(1000.0, 600.0);
const SETTINGS_OPTIONS: [SettingsOption; 3] = [
    SettingsOption::OpenDebugWindow,
    SettingsOption::HideMainWindow,
    SettingsOption::Quit,
];

enum InferenceState {
    Unloaded,
    Stopped,
    Running,
}

enum RunMode {
    Foreground,
    Background,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleIntervalChoice {
    Constant,
    Secs30,
    Min1,
    Min5,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsOption {
    OpenDebugWindow,
    HideMainWindow,
    Quit,
}

impl std::fmt::Display for SettingsOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::OpenDebugWindow => "Debug Window",
            Self::HideMainWindow => "Hide To Tray / Minimize",
            Self::Quit => "Quit App",
        };

        f.write_str(label)
    }
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
    alert_window_id: Option<window::Id>,
    tray_state: Option<tray::TrayState>,

    cam_frame: Option<image::Handle>,
    cv_frame: Option<image::Handle>,

    model_load_time: Option<Duration>,
    time_metrics: Option<TimeMetrics>,
    posture_angle_deg: Option<f32>,
    posture_baseline_deg: Option<f32>,
    posture_threshold_deg: f32,

    bad_posture: bool,
    inference_state: InferenceState,
    run_mode: RunMode,

    sample_interval_choice: SampleIntervalChoice,
    custom_interval_input: String,

    background_samples: Option<Vec<Option<f32>>>,
    last_alert_time: Option<Instant>,
    force_dismiss: bool,

    metrics: MetricsStore,
}

#[derive(Debug, Clone)]
pub enum Message {
    CamFrame(image::Handle),
    CvInference((image::Handle, TimeMetrics, Option<f32>)),
    WindowCloseRequested(window::Id),
    HideMainWindowPressed,
    RestoreMainWindowRequested,
    QuitRequested,
    SettingsOptionSelected(SettingsOption),
    OpenDebugWindowPressed,
    TestPosturePressed,
    StopInferencePressed,
    PostureThresholdChanged(f32),
    SampleIntervalChoiceChanged(SampleIntervalChoice),
    CustomIntervalInputChanged(String),
    ForceDismissToggled(bool),
    BackgroundSampleTick,
    DismissAlert,
}

impl App {
    fn new(pipelines: crate::Pipelines) -> (Self, Task<Message>) {
        let (main_window_id, open_main_window) = window::open(Self::main_window_settings());
        let tray_state = tray::TrayState::new()
            .map_err(|error| {
                eprintln!("Unable to initialize system tray: {error}");
                error
            })
            .ok();

        (
            Self {
                pipelines,
                main_window_id,
                debug_window_id: None,
                alert_window_id: None,
                tray_state,
                cam_frame: None,
                cv_frame: None,
                model_load_time: None,
                time_metrics: None,
                posture_angle_deg: None,
                posture_baseline_deg: None,
                posture_threshold_deg: DEFAULT_POSTURE_THRESHOLD_DEG,
                bad_posture: false,
                inference_state: InferenceState::Unloaded,
                run_mode: RunMode::Foreground,
                sample_interval_choice: SampleIntervalChoice::Min1,
                custom_interval_input: String::new(),
                background_samples: None,
                last_alert_time: None,
                force_dismiss: true,
                metrics: MetricsStore::new(),
            },
            open_main_window.discard(),
        )
    }

    fn sample_interval_secs(&self) -> Option<u64> {
        match self.sample_interval_choice {
            SampleIntervalChoice::Constant => None,
            SampleIntervalChoice::Secs30 => Some(30),
            SampleIntervalChoice::Min1 => Some(60),
            SampleIntervalChoice::Min5 => Some(300),
            SampleIntervalChoice::Custom => self
                .custom_interval_input
                .parse::<f32>()
                .ok()
                .filter(|&v| v > 0.0)
                .map(|v| (v * 60.0).max(1.0) as u64)
                .or(Some(60)),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CamFrame(frame) => {
                self.cam_frame = Some(frame);
                Task::none()
            }
            Message::CvInference((frame, time_metrics, posture_angle_deg)) => {
                self.cv_frame = Some(frame);
                self.time_metrics = Some(time_metrics);
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

                self.metrics.ingest(posture_angle_deg, self.bad_posture);

                if matches!(self.run_mode, RunMode::Background) {
                    if self.sample_interval_secs().is_none() {
                        // Constant mode: evaluate every incoming frame
                        if self.bad_posture {
                            let can_alert = self.alert_window_id.is_none()
                                && self
                                    .last_alert_time
                                    .map(|t| t.elapsed() >= ALERT_COOLDOWN)
                                    .unwrap_or(true);

                            if can_alert {
                                self.pipelines.camera_manager.stop();
                                self.pipelines.cv_manager.stop();
                                let (id, open) = window::open(Self::alert_window_settings());
                                self.alert_window_id = Some(id);
                                self.last_alert_time = Some(Instant::now());
                                return Task::batch([
                                    open.discard(),
                                    window::maximize(id, true),
                                ]);
                            }
                        } else if self.alert_window_id.is_some() && !self.force_dismiss {
                            return self.update(Message::DismissAlert);
                        }
                    } else {
                        // Timed mode: collect samples until we have enough
                        if let Some(ref mut samples) = self.background_samples {
                            samples.push(posture_angle_deg);
                        }

                        let has_enough = self
                            .background_samples
                            .as_ref()
                            .map(|s| s.len() >= BACKGROUND_SAMPLE_COUNT)
                            .unwrap_or(false);

                        if has_enough {
                            let baseline = self.posture_baseline_deg;
                            let threshold = self.posture_threshold_deg;

                            let bad_count = self
                                .background_samples
                                .as_ref()
                                .unwrap()
                                .iter()
                                .filter(|angle_opt| match (baseline, **angle_opt) {
                                    (Some(b), Some(a)) => (a - b).abs() >= threshold,
                                    _ => false,
                                })
                                .count();

                            self.background_samples = None;
                            self.pipelines.camera_manager.stop();
                            self.pipelines.cv_manager.stop();

                            if bad_count > BACKGROUND_SAMPLE_COUNT / 2 {
                                let can_alert = self.alert_window_id.is_none()
                                    && self
                                        .last_alert_time
                                        .map(|t| t.elapsed() >= ALERT_COOLDOWN)
                                        .unwrap_or(true);

                                if can_alert {
                                    let (id, open) =
                                        window::open(Self::alert_window_settings());
                                    self.alert_window_id = Some(id);
                                    self.last_alert_time = Some(Instant::now());
                                    return Task::batch([
                                        open.discard(),
                                        window::maximize(id, true),
                                    ]);
                                }
                            } else if self.alert_window_id.is_some() && !self.force_dismiss {
                                return self.update(Message::DismissAlert);
                            }
                        }
                    }
                }

                Task::none()
            }
            Message::WindowCloseRequested(window_id) => {
                if window_id == self.main_window_id {
                    if matches!(self.inference_state, InferenceState::Running) {
                        if self.sample_interval_secs().is_some() {
                            self.pipelines.camera_manager.stop();
                            self.pipelines.cv_manager.stop();
                        }
                        self.run_mode = RunMode::Background;
                        self.metrics.stop_tracking();
                    }
                    window::minimize(self.main_window_id, true)
                } else if self.debug_window_id == Some(window_id) {
                    self.debug_window_id = None;
                    window::close(window_id)
                } else if self.alert_window_id == Some(window_id) {
                    self.alert_window_id = None;
                    window::close(window_id)
                } else {
                    Task::none()
                }
            }
            Message::HideMainWindowPressed => {
                if matches!(self.inference_state, InferenceState::Running) {
                    if self.sample_interval_secs().is_some() {
                        self.pipelines.camera_manager.stop();
                        self.pipelines.cv_manager.stop();
                    }
                    self.run_mode = RunMode::Background;
                    self.metrics.stop_tracking();
                }
                window::minimize(self.main_window_id, true)
            }
            Message::RestoreMainWindowRequested => {
                let was_background = matches!(self.run_mode, RunMode::Background);
                let was_timed = self.sample_interval_secs().is_some();
                self.run_mode = RunMode::Foreground;
                self.background_samples = None;

                if was_background && was_timed && !self.pipelines.camera_manager.is_running() {
                    self.pipelines.camera_manager.start().ok();
                    self.pipelines.cv_manager.start().ok();
                }

                if was_background {
                    self.metrics.start_tracking();
                }

                Task::batch([
                    window::minimize(self.main_window_id, false),
                    window::gain_focus(self.main_window_id),
                ])
            }
            Message::QuitRequested => iced::exit(),
            Message::SettingsOptionSelected(option) => match option {
                SettingsOption::OpenDebugWindow => self.update(Message::OpenDebugWindowPressed),
                SettingsOption::HideMainWindow => self.update(Message::HideMainWindowPressed),
                SettingsOption::Quit => self.update(Message::QuitRequested),
            },
            Message::OpenDebugWindowPressed => {
                if self.debug_window_id.is_some() {
                    Task::none()
                } else {
                    let (debug_window_id, open_debug_window) =
                        window::open(Self::debug_window_settings());
                    self.debug_window_id = Some(debug_window_id);
                    open_debug_window.discard()
                }
            }
            Message::TestPosturePressed => {
                if matches!(self.inference_state, InferenceState::Unloaded) {
                    match self.pipelines.cv_manager.load_model() {
                        Ok(elapsed) => {
                            self.model_load_time = Some(elapsed);
                        }
                        Err(e) => {
                            eprintln!("Unable to load model: {}", e);
                            return Task::none();
                        }
                    };
                }

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
                self.run_mode = RunMode::Foreground;
                self.inference_state = InferenceState::Running;
                self.metrics.start_tracking();
                Task::none()
            }
            Message::StopInferencePressed => {
                self.pipelines.camera_manager.stop();
                self.pipelines.cv_manager.stop();
                self.bad_posture = false;
                self.inference_state = InferenceState::Stopped;
                self.run_mode = RunMode::Foreground;
                self.background_samples = None;
                self.metrics.stop_tracking();
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
            Message::SampleIntervalChoiceChanged(choice) => {
                let was_continuous = matches!(self.run_mode, RunMode::Background)
                    && self.sample_interval_secs().is_none();

                self.sample_interval_choice = choice;

                let is_continuous = self.sample_interval_secs().is_none();

                if matches!(self.run_mode, RunMode::Background) {
                    match (was_continuous, is_continuous) {
                        (false, true) => {
                            // Timed → Constant: restart workers so they run continuously
                            if !self.pipelines.camera_manager.is_running() {
                                self.pipelines.camera_manager.start().ok();
                                self.pipelines.cv_manager.start().ok();
                            }
                        }
                        (true, false) => {
                            // Constant → Timed: stop workers, wait for timer
                            self.pipelines.camera_manager.stop();
                            self.pipelines.cv_manager.stop();
                            self.background_samples = None;
                        }
                        _ => {}
                    }
                }

                Task::none()
            }
            Message::CustomIntervalInputChanged(input) => {
                // Auto-select Custom when the user types in the field
                self.sample_interval_choice = SampleIntervalChoice::Custom;
                self.custom_interval_input = input;
                Task::none()
            }
            Message::ForceDismissToggled(value) => {
                self.force_dismiss = value;
                Task::none()
            }
            Message::BackgroundSampleTick => {
                if self.alert_window_id.is_none() && !self.pipelines.camera_manager.is_running() {
                    self.pipelines.camera_manager.start().ok();
                    self.pipelines.cv_manager.start().ok();
                    self.background_samples = Some(Vec::new());
                }
                Task::none()
            }
            Message::DismissAlert => {
                if let Some(id) = self.alert_window_id.take() {
                    // Constant mode stops workers when showing the alert, so restart them now
                    if matches!(self.run_mode, RunMode::Background)
                        && self.sample_interval_secs().is_none()
                    {
                        self.pipelines.camera_manager.start().ok();
                        self.pipelines.cv_manager.start().ok();
                    }
                    window::close(id)
                } else {
                    Task::none()
                }
            }
        }
    }

    pub fn has_system_tray(&self) -> bool {
        self.tray_state.is_some()
    }

    pub fn has_debug_window(&self) -> bool {
        self.debug_window_id.is_some()
    }

    pub fn settings_options(&self) -> Vec<SettingsOption> {
        let mut options = Vec::with_capacity(SETTINGS_OPTIONS.len());

        for option in SETTINGS_OPTIONS {
            if option == SettingsOption::OpenDebugWindow && self.has_debug_window() {
                continue;
            }

            options.push(option);
        }

        options
    }

    pub fn background_action_label(&self) -> &'static str {
        if self.has_system_tray() {
            "Hide To Tray"
        } else {
            "Minimize Window"
        }
    }

    pub fn background_action_hint(&self) -> &'static str {
        if self.has_system_tray() {
            "Closing keeps the app available from the tray."
        } else {
            "Tray support is unavailable, so closing will only minimize."
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
        } else if self.alert_window_id == Some(window_id) {
            components::alert_overlay::view(self)
        } else {
            iced::widget::text("Unknown window").into()
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            subscriptions::raw_frame_subscription(self.pipelines.camera_manager.clone())
                .map(Message::CamFrame),
            subscriptions::inference_subscription(self.pipelines.cv_manager.clone())
                .map(Message::CvInference),
            window::close_requests().map(Message::WindowCloseRequested),
        ];

        if let Some(tray_state) = &self.tray_state {
            subscriptions.push(tray_state.subscription().map(|event| match event {
                tray::Event::OpenRequested => Message::RestoreMainWindowRequested,
                tray::Event::QuitRequested => Message::QuitRequested,
            }));
        }

        if matches!(self.run_mode, RunMode::Background) {
            if let Some(secs) = self.sample_interval_secs() {
                subscriptions.push(
                    iced::time::every(Duration::from_secs(secs))
                        .map(|_| Message::BackgroundSampleTick),
                );
            }
        }

        Subscription::batch(subscriptions)
    }

    fn theme(&self, _window_id: window::Id) -> Theme {
        theme::custom_theme()
    }

    fn title(&self, window_id: window::Id) -> String {
        if self.debug_window_id == Some(window_id) {
            "Debug Stats".to_string()
        } else if self.alert_window_id == Some(window_id) {
            "Posture Alert".to_string()
        } else {
            "PostureTracker".to_string()
        }
    }

    fn main_window_settings() -> window::Settings {
        window::Settings {
            size: MAIN_WINDOW_SIZE,
            resizable: false,
            minimizable: true,
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

    fn alert_window_settings() -> window::Settings {
        window::Settings {
            size: ALERT_WINDOW_SIZE,
            resizable: false,
            decorations: false,
            level: window::Level::AlwaysOnTop,
            position: window::Position::Centered,
            exit_on_close_request: false,
            ..Default::default()
        }
    }
}
