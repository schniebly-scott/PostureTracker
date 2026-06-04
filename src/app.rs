mod components;
mod subscriptions;
mod theme;
mod tray;

use std::time::{Duration, Instant};

use crate::config::{Config, PostureConfig};
use crate::cv::TimeMetrics;
use crate::metrics::MetricsStore;
use crate::utils::ManagedService;
use iced::widget::{column, container, image, row};
use iced::{Element, Length, Size, Subscription, Task, Theme, window};

const CALIBRATION_COUNTDOWN_SECS: u8 = 3;
const CALIBRATION_SAMPLE_SECS: u64 = 5;
const MIN_CALIBRATION_SAMPLES: usize = 5;
/// Floor for the alert cooldown. Below this the popup can reappear before the
/// user has had time to correct their posture after dismissing it.
pub const MIN_ALERT_COOLDOWN_SECS: u64 = 5;
const MAIN_WINDOW_SIZE: Size = Size::new(1206.0, 961.0);
const DEBUG_WINDOW_SIZE: Size = Size::new(720.0, 420.0);
const ALERT_WINDOW_SIZE: Size = Size::new(1000.0, 600.0);
const CONFIG_PATH: &str = "config.toml";

#[derive(PartialEq)]
enum InferenceState {
    Unloaded,
    Stopped,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Foreground,
    Background,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Dashboard,
    Settings,
}

pub enum CalibrationState {
    Idle,
    Countdown(u8),
    Sampling { samples: Vec<f32>, started_at: Instant },
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleIntervalChoice {
    Constant,
    Secs30,
    Min1,
    Min5,
    Custom,
}

pub const METRICS_TRANSITION_MS: u64 = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricsCategory {
    Daily,
    Session,
    AllTime,
    QuickView,
}

impl MetricsCategory {
    const ALL: [Self; 4] = [Self::Daily, Self::Session, Self::AllTime, Self::QuickView];

    pub fn label(self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::Session => "This Session",
            Self::AllTime => "All Time",
            Self::QuickView => "Quick View",
        }
    }

    pub fn cycle(self, dir: SlideDirection) -> Self {
        let i = Self::ALL.iter().position(|c| *c == self).unwrap_or(0);
        let n = Self::ALL.len();
        let next = match dir {
            SlideDirection::Right => (i + 1) % n,
            SlideDirection::Left => (i + n - 1) % n,
        };
        Self::ALL[next]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MetricsTransition {
    pub from: MetricsCategory,
    pub direction: SlideDirection,
    pub started_at: Instant,
}

impl MetricsTransition {
    pub fn progress(&self) -> f32 {
        let elapsed = self.started_at.elapsed().as_millis() as f32;
        (elapsed / METRICS_TRANSITION_MS as f32).clamp(0.0, 1.0)
    }

    pub fn is_done(&self) -> bool {
        self.started_at.elapsed().as_millis() as u64 >= METRICS_TRANSITION_MS
    }
}

pub fn run() -> iced::Result {
    iced::daemon(
        move || App::new(crate::new_app_state()),
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
    /// The mode the next session will start in, chosen via the control-panel
    /// toggle before pressing Start Session. Persisted to config.
    pub session_start_mode: RunMode,

    sample_interval_choice: SampleIntervalChoice,
    custom_interval_input: String,
    background_sample_count: usize,
    alert_cooldown: Duration,
    /// Raw text the user has typed into the cooldown field. `alert_cooldown` is
    /// only updated from this once it parses to a value at/above the floor.
    pub cooldown_input: String,

    background_samples: Option<Vec<Option<f32>>>,
    last_alert_time: Option<Instant>,
    force_dismiss: bool,

    calibration_state: CalibrationState,

    metrics: MetricsStore,
    pub metrics_category: MetricsCategory,
    pub metrics_transition: Option<MetricsTransition>,
    pub metrics_reset_open: bool,

    view: View,
    pub available_cameras: Vec<crate::camera::CameraOption>,
    pub camera_prompt_open: bool,

    config: Config,
}

#[derive(Debug, Clone)]
pub enum Message {
    CamFrame(image::Handle),
    CvInference((image::Handle, TimeMetrics, Option<f32>)),
    WindowCloseRequested(window::Id),
    HideMainWindowPressed,
    RestoreMainWindowRequested,
    QuitRequested,
    OpenDebugWindowPressed,
    TestPosturePressed,
    StopInferencePressed,
    PostureThresholdChanged(f32),
    PostureThresholdReleased,
    SampleIntervalChoiceChanged(SampleIntervalChoice),
    CustomIntervalInputChanged(String),
    CooldownInputChanged(String),
    ForceDismissToggled(bool),
    BackgroundSampleTick,
    DismissAlert,
    CalibratePressed,
    CalibrationTick,
    SessionModeSelected(RunMode),
    EnterBackgroundPressed,
    StartForegroundPressed,
    StopBackgroundPressed,
    MetricsCategoryCycled(SlideDirection),
    MetricsTransitionTick,
    MetricsResetMenuToggled,
    MetricsResetConfirmed,
    OpenSettingsPressed,
    CloseSettingsPressed,
    CameraSelected(crate::camera::CameraOption),
    CameraPromptConfirmed,
    RefreshCamerasPressed,
}

impl App {
    fn new((config, pipelines): (Config, crate::Pipelines)) -> (Self, Task<Message>) {
        let (main_window_id, open_main_window) = window::open(Self::main_window_settings());
        let tray_state = tray::TrayState::new()
            .map_err(|error| {
                eprintln!("Unable to initialize system tray: {error}");
                error
            })
            .ok();

        let (sample_interval_choice, custom_interval_input) =
            interval_choice_from_secs(config.background.interval_secs);

        let available_cameras = crate::camera::list_cameras();
        let camera_prompt_open = config.camera.device.is_none();

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
                posture_baseline_deg: config.posture.baseline_deg,
                posture_threshold_deg: config.posture.threshold_deg,
                bad_posture: false,
                inference_state: InferenceState::Unloaded,
                run_mode: RunMode::Foreground,
                session_start_mode: if config.session.start_in_background {
                    RunMode::Background
                } else {
                    RunMode::Foreground
                },
                sample_interval_choice,
                custom_interval_input,
                background_sample_count: config.background.frames_per_sample,
                alert_cooldown: Duration::from_secs(config.background.alert_cooldown_secs),
                cooldown_input: config.background.alert_cooldown_secs.to_string(),
                background_samples: None,
                last_alert_time: None,
                force_dismiss: config.background.force_dismiss,
                calibration_state: CalibrationState::Idle,
                metrics: MetricsStore::new(config.metrics.history_days_to_keep),
                metrics_category: MetricsCategory::Daily,
                metrics_transition: None,
                metrics_reset_open: false,
                view: View::Dashboard,
                available_cameras,
                camera_prompt_open,
                config,
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

    fn save_config(&mut self) {
        self.config.posture = PostureConfig {
            baseline_deg: self.posture_baseline_deg,
            threshold_deg: self.posture_threshold_deg,
        };
        self.config.background.interval_secs = match self.sample_interval_choice {
            SampleIntervalChoice::Constant => 0,
            SampleIntervalChoice::Secs30 => 30,
            SampleIntervalChoice::Min1 => 60,
            SampleIntervalChoice::Min5 => 300,
            SampleIntervalChoice::Custom => self.sample_interval_secs().unwrap_or(60),
        };
        self.config.background.force_dismiss = self.force_dismiss;
        self.config.background.alert_cooldown_secs = self.alert_cooldown.as_secs();
        if let Err(e) = self.config.save(CONFIG_PATH) {
            eprintln!("Failed to save config: {e}");
        }
    }

    pub fn is_calibrated(&self) -> bool {
        self.posture_baseline_deg.is_some()
    }

    pub fn is_background_mode(&self) -> bool {
        matches!(self.run_mode, RunMode::Background)
    }

    pub fn is_camera_running(&self) -> bool {
        self.pipelines.camera_manager.is_running()
    }

    pub fn mode_label(&self) -> &'static str {
        match self.run_mode {
            RunMode::Background => "Background",
            RunMode::Foreground => match self.inference_state {
                InferenceState::Running => "Testing",
                _ => "Idle",
            },
        }
    }

    /// Loads the model if needed, starts/stops the pipeline per the sample
    /// interval, and switches into background tracking. Returns `false` if the
    /// model failed to load (the caller should bail without changing windows).
    fn begin_background_tracking(&mut self) -> bool {
        if matches!(self.inference_state, InferenceState::Unloaded) {
            match self.pipelines.cv_manager.load_model() {
                Ok(elapsed) => {
                    self.model_load_time = Some(elapsed);
                }
                Err(e) => {
                    eprintln!("Unable to load model: {e}");
                    return false;
                }
            }
        }

        if self.sample_interval_secs().is_some() {
            self.pipelines.camera_manager.stop();
            self.pipelines.cv_manager.stop();
        } else if !self.pipelines.camera_manager.is_running() {
            self.pipelines.camera_manager.start().ok();
            self.pipelines.cv_manager.start().ok();
        }

        self.run_mode = RunMode::Background;
        self.calibration_state = CalibrationState::Idle;
        self.bad_posture = false;
        self.inference_state = InferenceState::Stopped;
        self.metrics.start_tracking();
        true
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CamFrame(frame) => {
                // Drop frames that were already in flight when the camera was
                // stopped, so a stale frame doesn't linger after a test ends.
                if !self.pipelines.camera_manager.is_running() {
                    return Task::none();
                }
                self.cam_frame = Some(frame);
                Task::none()
            }
            Message::CvInference((frame, time_metrics, posture_angle_deg)) => {
                if !self.pipelines.cv_manager.is_running() {
                    return Task::none();
                }
                self.cv_frame = Some(frame);
                self.time_metrics = Some(time_metrics);
                self.posture_angle_deg = posture_angle_deg;

                self.bad_posture = match (self.posture_baseline_deg, posture_angle_deg) {
                    (Some(baseline), Some(current)) => {
                        (current - baseline).abs() >= self.posture_threshold_deg
                    }
                    _ => false,
                };

                self.metrics.ingest(posture_angle_deg, self.bad_posture);

                // Calibration sample collection
                if let CalibrationState::Sampling { ref mut samples, started_at } =
                    self.calibration_state
                {
                    if let Some(angle) = posture_angle_deg {
                        samples.push(angle);
                    }

                    if started_at.elapsed() >= Duration::from_secs(CALIBRATION_SAMPLE_SECS) {
                        let collected = std::mem::take(samples);
                        if collected.len() < MIN_CALIBRATION_SAMPLES {
                            self.calibration_state = CalibrationState::Failed(format!(
                                "Only {}/{} valid samples — ensure your face and shoulders are visible.",
                                collected.len(),
                                MIN_CALIBRATION_SAMPLES
                            ));
                        } else {
                            let mut sorted = collected;
                            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                            let median = sorted[sorted.len() / 2];
                            self.posture_baseline_deg = Some(median);
                            self.calibration_state = CalibrationState::Idle;
                            self.save_config();
                        }
                    }
                }

                // Background mode posture checking
                if matches!(self.run_mode, RunMode::Background) {
                    if self.sample_interval_secs().is_none() {
                        if self.bad_posture {
                            let can_alert = self.alert_window_id.is_none()
                                && self
                                    .last_alert_time
                                    .map(|t| t.elapsed() >= self.alert_cooldown)
                                    .unwrap_or(true);

                            if can_alert {
                                // Leave the camera/CV pipeline running while the
                                // alert is shown so we keep evaluating posture and
                                // can auto-dismiss once it's corrected. The cooldown
                                // is started on dismiss, not here.
                                let (id, open) = window::open(Self::alert_window_settings());
                                self.alert_window_id = Some(id);
                                return Task::batch([open.discard(), window::maximize(id, true)]);
                            }
                        } else if self.alert_window_id.is_some() && !self.force_dismiss {
                            return self.update(Message::DismissAlert);
                        }
                    } else {
                        if let Some(ref mut samples) = self.background_samples {
                            samples.push(posture_angle_deg);
                        }

                        let has_enough = self
                            .background_samples
                            .as_ref()
                            .map(|s| s.len() >= self.background_sample_count)
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

                            if bad_count > self.background_sample_count / 2 {
                                let can_alert = self.alert_window_id.is_none()
                                    && self
                                        .last_alert_time
                                        .map(|t| t.elapsed() >= self.alert_cooldown)
                                        .unwrap_or(true);

                                if can_alert {
                                    let (id, open) = window::open(Self::alert_window_settings());
                                    self.alert_window_id = Some(id);
                                    return Task::batch([open.discard(), window::maximize(id, true)]);
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
                    return self.update(Message::QuitRequested);
                } else if self.debug_window_id == Some(window_id) {
                    self.debug_window_id = None;
                    window::close(window_id)
                } else if self.alert_window_id == Some(window_id) {
                    self.alert_window_id = None;
                    self.last_alert_time = Some(Instant::now());
                    window::close(window_id)
                } else {
                    Task::none()
                }
            }
            Message::HideMainWindowPressed => {
                window::minimize(self.main_window_id, true)
            }
            Message::RestoreMainWindowRequested => {
                Task::batch([
                    window::minimize(self.main_window_id, false),
                    window::gain_focus(self.main_window_id),
                ])
            }
            Message::QuitRequested => iced::exit(),
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
                self.bad_posture = false;
                self.run_mode = RunMode::Foreground;
                self.inference_state = InferenceState::Running;
                Task::none()
            }
            Message::StopInferencePressed => {
                self.pipelines.camera_manager.stop();
                self.pipelines.cv_manager.stop();
                self.cam_frame = None;
                self.cv_frame = None;
                self.bad_posture = false;
                self.inference_state = InferenceState::Stopped;
                self.run_mode = RunMode::Foreground;
                self.background_samples = None;
                self.calibration_state = CalibrationState::Idle;
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
            Message::PostureThresholdReleased => {
                self.save_config();
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
                            if !self.pipelines.camera_manager.is_running() {
                                self.pipelines.camera_manager.start().ok();
                                self.pipelines.cv_manager.start().ok();
                            }
                        }
                        (true, false) => {
                            self.pipelines.camera_manager.stop();
                            self.pipelines.cv_manager.stop();
                            self.background_samples = None;
                        }
                        _ => {}
                    }
                }

                self.save_config();
                Task::none()
            }
            Message::CustomIntervalInputChanged(input) => {
                self.sample_interval_choice = SampleIntervalChoice::Custom;
                self.custom_interval_input = input;
                self.save_config();
                Task::none()
            }
            Message::CooldownInputChanged(input) => {
                self.cooldown_input = input;
                // Only commit values at/above the floor. Too-low or invalid
                // input keeps the last valid cooldown while the settings page
                // shows a warning.
                if let Ok(secs) = self.cooldown_input.trim().parse::<u64>() {
                    if secs >= MIN_ALERT_COOLDOWN_SECS {
                        self.alert_cooldown = Duration::from_secs(secs);
                        self.save_config();
                    }
                }
                Task::none()
            }
            Message::ForceDismissToggled(value) => {
                self.force_dismiss = value;
                self.save_config();
                Task::none()
            }
            Message::BackgroundSampleTick => {
                // Keep sampling while an alert is open only when auto-dismiss is
                // enabled, so the next sample can detect corrected posture and
                // close the alert. With manual dismissal we idle the camera until
                // the user clicks.
                let alert_blocks_sampling = self.alert_window_id.is_some() && self.force_dismiss;
                if !alert_blocks_sampling && !self.pipelines.camera_manager.is_running() {
                    self.pipelines.camera_manager.start().ok();
                    self.pipelines.cv_manager.start().ok();
                    self.background_samples = Some(Vec::new());
                }
                Task::none()
            }
            Message::DismissAlert => {
                if let Some(id) = self.alert_window_id.take() {
                    // Start the re-alert cooldown from when the alert is
                    // dismissed, not when it opened. The pipeline was never
                    // stopped on open, so there's nothing to restart here.
                    self.last_alert_time = Some(Instant::now());
                    window::close(id)
                } else {
                    Task::none()
                }
            }
            Message::SessionModeSelected(mode) => {
                self.session_start_mode = mode;
                self.config.session.start_in_background = matches!(mode, RunMode::Background);
                self.save_config();
                Task::none()
            }
            Message::EnterBackgroundPressed => {
                if self.begin_background_tracking() {
                    window::minimize(self.main_window_id, true)
                } else {
                    Task::none()
                }
            }
            Message::StartForegroundPressed => {
                // Same tracking start as Enter Background, but keep the window
                // visible and focused instead of minimizing it.
                if self.begin_background_tracking() {
                    window::gain_focus(self.main_window_id)
                } else {
                    Task::none()
                }
            }
            Message::StopBackgroundPressed => {
                self.pipelines.camera_manager.stop();
                self.pipelines.cv_manager.stop();
                self.cam_frame = None;
                self.cv_frame = None;
                self.run_mode = RunMode::Foreground;
                self.inference_state = InferenceState::Stopped;
                self.background_samples = None;
                self.metrics.stop_tracking();
                Task::none()
            }
            Message::CalibratePressed => {
                self.calibration_state = CalibrationState::Countdown(CALIBRATION_COUNTDOWN_SECS);
                Task::none()
            }
            Message::CalibrationTick => {
                match self.calibration_state {
                    CalibrationState::Countdown(1) => {
                        self.calibration_state = CalibrationState::Sampling {
                            samples: Vec::new(),
                            started_at: Instant::now(),
                        };
                    }
                    CalibrationState::Countdown(n) => {
                        self.calibration_state = CalibrationState::Countdown(n - 1);
                    }
                    _ => {}
                }
                Task::none()
            }
            Message::MetricsCategoryCycled(direction) => {
                let from = self.metrics_category;
                self.metrics_category = from.cycle(direction);
                self.metrics_transition = Some(MetricsTransition {
                    from,
                    direction,
                    started_at: Instant::now(),
                });
                self.metrics_reset_open = false;
                Task::none()
            }
            Message::MetricsTransitionTick => {
                if let Some(t) = self.metrics_transition {
                    if t.is_done() {
                        self.metrics_transition = None;
                    }
                }
                Task::none()
            }
            Message::MetricsResetMenuToggled => {
                self.metrics_reset_open = !self.metrics_reset_open;
                Task::none()
            }
            Message::MetricsResetConfirmed => {
                match self.metrics_category {
                    MetricsCategory::Daily => self.metrics.reset_today(),
                    MetricsCategory::Session => self.metrics.reset_session(),
                    MetricsCategory::AllTime => self.metrics.reset_all_time(),
                    MetricsCategory::QuickView => self.metrics.reset_session(),
                }
                self.metrics_reset_open = false;
                Task::none()
            }
            Message::OpenSettingsPressed => {
                self.available_cameras = crate::camera::list_cameras();
                self.view = View::Settings;
                Task::none()
            }
            Message::CloseSettingsPressed => {
                self.view = View::Dashboard;
                Task::none()
            }
            Message::RefreshCamerasPressed => {
                self.available_cameras = crate::camera::list_cameras();
                Task::none()
            }
            Message::CameraSelected(option) => {
                self.config.camera.device = Some(option.path.clone());
                self.save_config();
                self.pipelines.camera_manager.set_device(Some(option.path));
                // Apply immediately if a session is live so the feed switches
                // to the newly chosen device.
                if self.pipelines.camera_manager.is_running() {
                    self.pipelines.camera_manager.stop();
                    if let Err(e) = self.pipelines.camera_manager.start() {
                        eprintln!("Failed to restart camera: {e}");
                    }
                }
                Task::none()
            }
            Message::CameraPromptConfirmed => {
                if self.config.camera.device.is_some() {
                    self.camera_prompt_open = false;
                }
                Task::none()
            }
        }
    }

    pub fn has_system_tray(&self) -> bool {
        self.tray_state.is_some()
    }

    pub fn has_debug_window(&self) -> bool {
        self.debug_window_id.is_some()
    }

    fn view(&self, window_id: window::Id) -> Element<'_, Message> {
        if window_id == self.main_window_id {
            let content: Element<'_, Message> = match self.view {
                View::Dashboard => {
                    let body = column![
                        row![
                            components::camera_panel::view(self),
                            column![
                                components::control_panel::view(self),
                                components::metrics_panel::view(self),
                            ]
                            .spacing(14)
                            .width(Length::Fixed(462.0)),
                        ]
                        .spacing(14)
                        .height(Length::FillPortion(7)),
                        components::status_panel::view(self),
                    ]
                    .spacing(14);

                    container(body).padding(14).height(Length::Fill).into()
                }
                View::Settings => components::settings_panel::view(self),
            };

            if self.camera_prompt_open {
                iced::widget::stack![content, components::settings_panel::camera_prompt(self)]
                    .into()
            } else {
                content
            }
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

        if matches!(self.calibration_state, CalibrationState::Countdown(_)) {
            subscriptions.push(
                iced::time::every(Duration::from_secs(1))
                    .map(|_| Message::CalibrationTick),
            );
        }

        if self.metrics_transition.is_some() {
            subscriptions.push(
                iced::time::every(Duration::from_millis(16))
                    .map(|_| Message::MetricsTransitionTick),
            );
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

fn interval_choice_from_secs(secs: u64) -> (SampleIntervalChoice, String) {
    match secs {
        0 => (SampleIntervalChoice::Constant, String::new()),
        30 => (SampleIntervalChoice::Secs30, String::new()),
        60 => (SampleIntervalChoice::Min1, String::new()),
        300 => (SampleIntervalChoice::Min5, String::new()),
        other => {
            let mins = other as f64 / 60.0;
            (SampleIntervalChoice::Custom, format!("{mins}"))
        }
    }
}
