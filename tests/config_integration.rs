//! Integration tests for the public config API: save/load round-trips and the
//! `#[serde(default)]` fallbacks, exercised against real files in temp dirs.

use posturetracker::config::Config;
use tempfile::tempdir;

#[test]
fn save_then_load_round_trips_non_default_values() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");

    let mut cfg = Config::default();
    cfg.posture.baseline_deg = Some(10.0);
    cfg.posture.threshold_deg = 4.5;
    cfg.background.interval_secs = 0;
    cfg.background.alert_cooldown_secs = 5;
    cfg.camera.device = Some("/dev/video2".to_string());
    cfg.session.start_in_background = true;

    cfg.save(&path).expect("save should succeed");

    let loaded = Config::load_or_default(&path);
    assert_eq!(loaded.posture.baseline_deg, Some(10.0));
    assert_eq!(loaded.posture.threshold_deg, 4.5);
    assert_eq!(loaded.background.interval_secs, 0);
    assert_eq!(loaded.background.alert_cooldown_secs, 5);
    assert_eq!(loaded.camera.device, Some("/dev/video2".to_string()));
    assert!(loaded.session.start_in_background);
}

#[test]
fn missing_file_yields_defaults() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("absent.toml");

    let cfg = Config::load_or_default(&path);
    assert_eq!(cfg.posture.threshold_deg, 12.0);
    assert_eq!(cfg.background.interval_secs, 60);
}

#[test]
fn partial_file_fills_missing_sections_with_defaults() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("partial.toml");
    std::fs::write(&path, "[posture]\nthreshold_deg = 4.5\n").unwrap();

    let cfg = Config::load_or_default(&path);
    assert_eq!(cfg.posture.threshold_deg, 4.5); // from file
    assert_eq!(cfg.background.interval_secs, 60); // defaulted
    assert_eq!(cfg.metrics.history_days_to_keep, 30); // defaulted
}

#[test]
fn corrupt_file_falls_back_to_defaults() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("corrupt.toml");
    std::fs::write(&path, "this is not = valid toml {{{").unwrap();

    let cfg = Config::load_or_default(&path);
    assert_eq!(cfg.posture.threshold_deg, 12.0);
}

#[test]
fn unset_camera_device_is_omitted_from_serialized_output() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");

    // Default config has camera.device == None.
    Config::default().save(&path).unwrap();

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(
        !contents.contains("device"),
        "device should be skipped when None, got:\n{contents}"
    );
}
