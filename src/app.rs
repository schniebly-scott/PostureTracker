mod components;
mod subscriptions;
mod theme;
mod tray;

use std::time::{Duration, Instant};

use crate::config::{config_path, Config, PostureConfig};
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
const MAIN_WINDOW_SIZE: Size = Size::new(1100.0, 860.0);
/// Floor that keeps every panel reachable on small/scaled displays (e.g. a
/// 1366×768 panel, or a 1080p screen at 150 % giving 1280×720 logical px).
const MAIN_WINDOW_MIN_SIZE: Size = Size::new(980.0, 640.0);
const DEBUG_WINDOW_SIZE: Size = Size::new(720.0, 420.0);
const ALERT_WINDOW_SIZE: Size = Size::new(1000.0, 600.0);

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
    use components::ui;

    iced::daemon(
        move || App::new(crate::new_app_state()),
        App::update,
        App::view,
    )
    .title(App::title)
    .subscription(App::subscription)
    .theme(App::theme)
    // Bundle every face we render with so glyphs resolve identically on all
    // platforms instead of via per-OS system-font fallback. Inter is the
    // default text font; numbers use JetBrains Mono and icons the subset
    // icon font (see `components::ui`).
    .default_font(ui::INTER)
    .font(ui::INTER_REGULAR)
    .font(ui::INTER_SEMIBOLD)
    .font(ui::INTER_BOLD)
    .font(ui::JETBRAINS_MONO)
    .font(ui::ICON_FONT)
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

    /// Set when a text field (custom interval, alert cooldown) has accepted a
    /// keystroke whose value isn't on disk yet. Keystrokes only mutate
    /// in-memory state; the write is deferred to a commit point (Enter,
    /// leaving the field's page, interval choice change, or quit) where
    /// `flush_config` runs. Mirrors the slider's edit-then-commit pattern and
    /// avoids a full TOML rewrite per keystroke.
    config_dirty: bool,

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
    /// Persist deferred text-field edits (fired on Enter in a text input).
    CommitConfig,
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
                config_dirty: false,
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
        if let Err(e) = self.config.save(config_path()) {
            eprintln!("Failed to save config: {e}");
        }
        // Whatever was pending is now on disk (re-derived above).
        self.config_dirty = false;
    }

    /// Write config only if a deferred text-field edit is pending. Called at
    /// commit points (Enter, leaving the settings page, quitting) so deferred
    /// edits aren't lost without forcing a write when nothing changed.
    fn flush_config(&mut self) {
        if self.config_dirty {
            self.save_config();
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

    /// Loads the inference model if it hasn't been loaded yet, recording the
    /// load time. Returns `false` if the model failed to load (the caller
    /// should bail out without starting the pipeline).
    fn ensure_model_loaded(&mut self) -> bool {
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
        true
    }

    /// Loads the model if needed, starts/stops the pipeline per the sample
    /// interval, and switches into background tracking. Returns `false` if the
    /// model failed to load (the caller should bail without changing windows).
    fn begin_background_tracking(&mut self) -> bool {
        if !self.ensure_model_loaded() {
            return false;
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

    /// Ingests a fresh inference result: updates the displayed frame, time
    /// metrics, and current angle, recomputes `bad_posture`, and feeds the
    /// metrics store.
    fn apply_inference(
        &mut self,
        frame: image::Handle,
        time_metrics: TimeMetrics,
        angle: Option<f32>,
    ) {
        self.cv_frame = Some(frame);
        self.time_metrics = Some(time_metrics);
        self.posture_angle_deg = angle;
        self.bad_posture =
            is_bad_posture(self.posture_baseline_deg, angle, self.posture_threshold_deg);
        self.metrics.ingest(angle, self.bad_posture);
    }

    /// Advances an in-progress calibration by one sample. Pushes the angle (if
    /// present) and, once the sampling window has elapsed, resolves to a
    /// baseline (saving config) or a failure message. A no-op when not
    /// currently sampling.
    fn ingest_calibration_sample(&mut self, angle: Option<f32>) {
        if let CalibrationState::Sampling {
            ref mut samples,
            started_at,
        } = self.calibration_state
        {
            if let Some(angle) = angle {
                samples.push(angle);
            }

            if started_at.elapsed() >= Duration::from_secs(CALIBRATION_SAMPLE_SECS) {
                let collected = std::mem::take(samples);
                self.calibration_state = match calibration_baseline(collected) {
                    Ok(median) => {
                        self.posture_baseline_deg = Some(median);
                        self.save_config();
                        CalibrationState::Idle
                    }
                    Err(message) => CalibrationState::Failed(message),
                };
            }
        }
    }

    /// Background-mode posture policy. In continuous mode every frame is judged
    /// directly; in interval mode frames are buffered and a majority vote
    /// decides. Returns the Task to run (possibly opening or dismissing the
    /// alert window).
    fn evaluate_background(&mut self, angle: Option<f32>) -> Task<Message> {
        if !matches!(self.run_mode, RunMode::Background) {
            return Task::none();
        }

        if self.sample_interval_secs().is_none() {
            // Continuous mode: judge each frame as it arrives. The pipeline is
            // intentionally left running while an alert is shown (see
            // `evaluate_alert`) so corrected posture can auto-dismiss it.
            return self.evaluate_alert(self.bad_posture);
        }

        // Interval mode: buffer frames until we have a full sample window.
        if let Some(ref mut samples) = self.background_samples {
            samples.push(angle);
        }

        let has_enough = self
            .background_samples
            .as_ref()
            .map(|s| s.len() >= self.background_sample_count)
            .unwrap_or(false);

        if !has_enough {
            return Task::none();
        }

        let majority_bad = majority_bad_posture(
            self.background_samples.as_deref().unwrap_or(&[]),
            self.posture_baseline_deg,
            self.posture_threshold_deg,
            self.background_sample_count,
        );

        // Unlike continuous mode, interval mode stops the pipeline before
        // deciding to alert — the camera idles between samples and is
        // restarted by the next `BackgroundSampleTick`.
        self.background_samples = None;
        self.pipelines.camera_manager.stop();
        self.pipelines.cv_manager.stop();

        self.evaluate_alert(majority_bad)
    }

    /// Single policy point for the alert window. Opens (and maximizes) the
    /// alert if posture is bad and the cooldown allows; auto-dismisses an open
    /// alert when posture is good unless `force_dismiss` requires a manual
    /// close. Returns the Task to run.
    fn evaluate_alert(&mut self, bad: bool) -> Task<Message> {
        if bad {
            let can_alert = self.alert_window_id.is_none()
                && self
                    .last_alert_time
                    .map(|t| t.elapsed() >= self.alert_cooldown)
                    .unwrap_or(true);

            if can_alert {
                // The cooldown is started on dismiss, not here.
                let (id, open) = window::open(Self::alert_window_settings());
                self.alert_window_id = Some(id);
                return Task::batch([open.discard(), window::maximize(id, true)]);
            }
        } else if self.alert_window_id.is_some() && !self.force_dismiss {
            return self.dismiss_alert();
        }

        Task::none()
    }

    /// Closes the alert window if one is open and starts the re-alert cooldown
    /// from now. The pipeline is never stopped on open, so there's nothing to
    /// restart here.
    fn dismiss_alert(&mut self) -> Task<Message> {
        if let Some(id) = self.alert_window_id.take() {
            self.last_alert_time = Some(Instant::now());
            window::close(id)
        } else {
            Task::none()
        }
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
                self.apply_inference(frame, time_metrics, posture_angle_deg);
                self.ingest_calibration_sample(posture_angle_deg);
                self.evaluate_background(posture_angle_deg)
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
            Message::QuitRequested => {
                // Final flush for any text-field edit not yet committed.
                self.flush_config();
                iced::exit()
            }
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
                if !self.ensure_model_loaded() {
                    return Task::none();
                }

                if let Err(e) = self.pipelines.camera_manager.start() {
                    eprintln!("Unable to start camera: {e}");
                }
                if let Err(e) = self.pipelines.cv_manager.start() {
                    eprintln!("Unable to start model: {e}");
                }
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
                self.bad_posture = is_bad_posture(
                    self.posture_baseline_deg,
                    self.posture_angle_deg,
                    self.posture_threshold_deg,
                );
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
                // Keep keystrokes in-memory; persist at a commit point so we
                // don't rewrite config (and rebuild the timer subscription) on
                // every character, and don't persist transient values like the
                // 60s fallback while the field reads "1".
                self.config_dirty = true;
                Task::none()
            }
            Message::CooldownInputChanged(input) => {
                self.cooldown_input = input;
                // Only accept values at/above the floor. Too-low or invalid
                // input keeps the last valid cooldown while the settings page
                // shows a warning. The accepted value is held in memory and
                // written at a commit point.
                if let Ok(secs) = self.cooldown_input.trim().parse::<u64>() {
                    if secs >= MIN_ALERT_COOLDOWN_SECS {
                        self.alert_cooldown = Duration::from_secs(secs);
                        self.config_dirty = true;
                    }
                }
                Task::none()
            }
            Message::CommitConfig => {
                self.flush_config();
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
            Message::DismissAlert => self.dismiss_alert(),
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
                    // QuickView surfaces today's metrics (breaks today, streak,
                    // posture quality today), so it must reset the daily buckets.
                    MetricsCategory::QuickView => self.metrics.reset_today(),
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
                // Leaving the settings page commits the cooldown field.
                self.flush_config();
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
                            // Proportional against the camera panel's
                            // FillPortion(3) so the columns reflow with the
                            // window instead of pinning a 462 px width.
                            .width(Length::FillPortion(2)),
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
            // Resizable + a guaranteed floor so the dashboard fits—and stays
            // usable—on small or scaled screens. The main window is the
            // dashboard, not an alert, so it stays at the normal stacking
            // level instead of AlwaysOnTop (that's reserved for the alert).
            min_size: Some(MAIN_WINDOW_MIN_SIZE),
            resizable: true,
            minimizable: true,
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

/// Posture is "bad" when the current angle deviates from the calibrated
/// baseline by at least the threshold. Missing baseline or angle reads as good
/// posture (nothing to compare against).
fn is_bad_posture(baseline: Option<f32>, angle: Option<f32>, threshold: f32) -> bool {
    match (baseline, angle) {
        (Some(baseline), Some(angle)) => (angle - baseline).abs() >= threshold,
        _ => false,
    }
}

/// Interval-mode decision: do a majority of the buffered samples read as bad
/// posture? The vote is taken against the expected `sample_count` (the window
/// size), so a window padded with extra frames still needs a true majority.
fn majority_bad_posture(
    samples: &[Option<f32>],
    baseline: Option<f32>,
    threshold: f32,
    sample_count: usize,
) -> bool {
    let bad_count = samples
        .iter()
        .filter(|angle| is_bad_posture(baseline, **angle, threshold))
        .count();
    bad_count > sample_count / 2
}

/// Resolves a completed calibration window to a baseline angle. Requires at
/// least `MIN_CALIBRATION_SAMPLES` valid samples; on success returns their
/// median, otherwise an error message suitable for display.
fn calibration_baseline(samples: Vec<f32>) -> Result<f32, String> {
    if samples.len() < MIN_CALIBRATION_SAMPLES {
        return Err(format!(
            "Only {}/{} valid samples — ensure your face and shoulders are visible.",
            samples.len(),
            MIN_CALIBRATION_SAMPLES
        ));
    }

    let mut sorted = samples;
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Ok(sorted[sorted.len() / 2])
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_category_labels() {
        assert_eq!(MetricsCategory::Daily.label(), "Daily");
        assert_eq!(MetricsCategory::Session.label(), "This Session");
        assert_eq!(MetricsCategory::AllTime.label(), "All Time");
        assert_eq!(MetricsCategory::QuickView.label(), "Quick View");
    }

    #[test]
    fn metrics_category_cycle_wraps_full_circle() {
        // Stepping Right through all four variants returns to the start.
        let mut c = MetricsCategory::Daily;
        for _ in 0..4 {
            c = c.cycle(SlideDirection::Right);
        }
        assert_eq!(c, MetricsCategory::Daily);
    }

    #[test]
    fn metrics_category_cycle_left_is_inverse_of_right() {
        let c = MetricsCategory::Session;
        let round_trip = c.cycle(SlideDirection::Right).cycle(SlideDirection::Left);
        assert_eq!(round_trip, c);
    }

    #[test]
    fn metrics_category_cycle_direction_order() {
        assert_eq!(
            MetricsCategory::Daily.cycle(SlideDirection::Right),
            MetricsCategory::Session
        );
        assert_eq!(
            MetricsCategory::Daily.cycle(SlideDirection::Left),
            MetricsCategory::QuickView
        );
    }

    #[test]
    fn interval_choice_maps_known_intervals() {
        assert_eq!(interval_choice_from_secs(0).0, SampleIntervalChoice::Constant);
        assert_eq!(interval_choice_from_secs(30).0, SampleIntervalChoice::Secs30);
        assert_eq!(interval_choice_from_secs(60).0, SampleIntervalChoice::Min1);
        assert_eq!(interval_choice_from_secs(300).0, SampleIntervalChoice::Min5);
    }

    #[test]
    fn interval_choice_custom_carries_minutes_string() {
        let (choice, text) = interval_choice_from_secs(120);
        assert_eq!(choice, SampleIntervalChoice::Custom);
        assert_eq!(text, "2");
    }

    #[test]
    fn is_bad_posture_needs_baseline_and_angle() {
        // No baseline or no current angle => can't judge => not bad.
        assert!(!is_bad_posture(None, Some(20.0), 5.0));
        assert!(!is_bad_posture(Some(10.0), None, 5.0));
        assert!(!is_bad_posture(None, None, 5.0));
    }

    #[test]
    fn is_bad_posture_compares_deviation_to_threshold() {
        let baseline = Some(10.0);
        // Deviation below threshold is fine.
        assert!(!is_bad_posture(baseline, Some(14.0), 5.0));
        // At-threshold is bad (>=), in either direction.
        assert!(is_bad_posture(baseline, Some(15.0), 5.0));
        assert!(is_bad_posture(baseline, Some(5.0), 5.0));
        // Well past the threshold.
        assert!(is_bad_posture(baseline, Some(30.0), 5.0));
    }

    #[test]
    fn majority_bad_posture_needs_strict_majority() {
        let baseline = Some(0.0);
        let threshold = 5.0;
        // good, good, bad, bad => 2 of 4, not a strict majority of 4.
        let samples = [Some(0.0), Some(1.0), Some(10.0), Some(20.0)];
        assert!(!majority_bad_posture(&samples, baseline, threshold, 4));
        // good, bad, bad, bad => 3 of 4 is a majority.
        let samples = [Some(0.0), Some(10.0), Some(20.0), Some(30.0)];
        assert!(majority_bad_posture(&samples, baseline, threshold, 4));
    }

    #[test]
    fn majority_bad_posture_counts_missing_angles_as_good() {
        let baseline = Some(0.0);
        // Two bad reads but two missing => not a majority of 4.
        let samples = [None, None, Some(10.0), Some(20.0)];
        assert!(!majority_bad_posture(&samples, baseline, 5.0, 4));
    }

    #[test]
    fn majority_bad_posture_votes_against_expected_window() {
        // An overfilled window (5 samples) still needs a majority of the
        // expected count (4), i.e. > 2.
        let baseline = Some(0.0);
        let samples = [Some(0.0), Some(0.0), Some(10.0), Some(10.0), Some(10.0)];
        assert!(majority_bad_posture(&samples, baseline, 5.0, 4));
    }

    #[test]
    fn calibration_baseline_rejects_too_few_samples() {
        let samples = vec![1.0; MIN_CALIBRATION_SAMPLES - 1];
        assert!(calibration_baseline(samples).is_err());
    }

    #[test]
    fn calibration_baseline_returns_median() {
        // Unsorted input; median of 7 sorted values is the 4th (index 3).
        let samples = vec![30.0, 10.0, 20.0, 50.0, 40.0, 70.0, 60.0];
        assert_eq!(calibration_baseline(samples), Ok(40.0));
    }
}
