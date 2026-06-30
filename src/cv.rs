mod cv_inference;
mod cv_service;
mod cv_worker;
mod pose;

use std::time::Duration;

use crate::camera::Frame;
pub use cv_service::CVManager;

#[derive(Clone, Debug)]
pub struct Inference {
    pub frame: Frame,
    pub time_metrics: TimeMetrics,
    pub posture_angle_deg: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct TimeMetrics {
    pub preprocess: Duration,
    pub inference: Duration,
    pub postprocess: Duration,
    pub render: Duration,
}
