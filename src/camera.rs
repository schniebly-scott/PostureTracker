mod cam_service;
mod cam_worker;

use std::sync::Arc;

pub use cam_service::{CameraManager, RgbaBuffer};

/// RGBA frame sent to the UI
/// (width, height, RgbaBuffer { frame, pool-pointer })
pub type Frame = (u32, u32, Arc<RgbaBuffer>);