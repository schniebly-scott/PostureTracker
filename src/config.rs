use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub camera: CameraConfig,
    #[serde(default)]
    pub posture: PostureConfig,
    #[serde(default)]
    pub background: BackgroundConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub session: SessionConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SessionConfig {
    /// How the next session starts. `false` (the default) means Foreground —
    /// the window stays visible; `true` means Background — the window minimizes.
    #[serde(default)]
    pub start_in_background: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CameraConfig {
    /// Device name to capture from. `None` means no camera has been chosen yet,
    /// which triggers the first-run selection prompt. Omitted from the TOML file
    /// when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostureConfig {
    pub baseline_deg: Option<f32>,
    #[serde(default = "PostureConfig::default_threshold")]
    pub threshold_deg: f32,
}

impl PostureConfig {
    fn default_threshold() -> f32 { 12.0 }
}

impl Default for PostureConfig {
    fn default() -> Self {
        Self { baseline_deg: None, threshold_deg: Self::default_threshold() }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackgroundConfig {
    /// Seconds between background checks; 0 means continuous.
    #[serde(default = "BackgroundConfig::default_interval_secs")]
    pub interval_secs: u64,
    #[serde(default = "BackgroundConfig::default_frames_per_sample")]
    pub frames_per_sample: usize,
    #[serde(default = "BackgroundConfig::default_alert_cooldown_secs")]
    pub alert_cooldown_secs: u64,
    #[serde(default = "BackgroundConfig::default_force_dismiss")]
    pub force_dismiss: bool,
}

impl BackgroundConfig {
    fn default_interval_secs() -> u64 { 60 }
    fn default_frames_per_sample() -> usize { 3 }
    fn default_alert_cooldown_secs() -> u64 { 60 }
    fn default_force_dismiss() -> bool { true }
}

impl Default for BackgroundConfig {
    fn default() -> Self {
        Self {
            interval_secs: Self::default_interval_secs(),
            frames_per_sample: Self::default_frames_per_sample(),
            alert_cooldown_secs: Self::default_alert_cooldown_secs(),
            force_dismiss: Self::default_force_dismiss(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetricsConfig {
    #[serde(default = "MetricsConfig::default_history_days")]
    pub history_days_to_keep: i64,
}

impl MetricsConfig {
    fn default_history_days() -> i64 { 30 }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self { history_days_to_keep: Self::default_history_days() }
    }
}

impl Config {
    /// Load from disk, falling back to defaults if the file is missing or unparseable.
    /// Missing sections within an existing file are filled by `#[serde(default)]`.
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        let Ok(contents) = fs::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str(&contents).unwrap_or_default()
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let toml = toml::to_string_pretty(self)?;
        fs::write(path, toml)?;
        Ok(())
    }
}
