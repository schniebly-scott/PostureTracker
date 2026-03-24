mod cv_inference;
mod cv_service;
mod cv_worker;
mod pose;

use std::time::Duration;

use crate::camera::Frame;
pub use cv_service::CVManager;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct Inference {
    pub frame: Frame,
    pub inf_time: Duration,
    pub posture_angle_deg: Option<f32>,
}

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub enum InfType {
    Pose,
    BoundingBox,
    Segment,
}
