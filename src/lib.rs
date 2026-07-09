pub mod app;
pub mod camera;
pub mod config;
pub mod constants;
pub mod cv;
pub mod frame_channel;
pub mod metrics;
pub mod utils;

use std::sync::Arc;

use camera::{CameraManager, Frame};
use config::Config;
use cv::CVManager;
use frame_channel::FrameChannel;
use utils::PipelineErrors;

pub use app::run;

pub type SharedFrame = Arc<FrameChannel>;

#[derive(Clone, Debug)]
pub struct Pipelines {
    pub camera_manager: Arc<CameraManager>,
    pub cv_manager: Arc<CVManager>,
    /// Fan-in for worker-thread failures; the UI subscribes and surfaces the
    /// latest one as a dismissible banner in the status panel.
    pub errors: PipelineErrors,
}

pub fn new_app_state() -> (Config, Pipelines) {
    let config = Config::load_or_default(config::config_path());

    let errors = PipelineErrors::new();
    let shared_frame: SharedFrame = Arc::new(FrameChannel::new());
    let camera_manager = Arc::new(CameraManager::new(
        config.camera.clone(),
        shared_frame.clone(),
        errors.clone(),
    ));
    let cv_manager = Arc::new(CVManager::new(shared_frame.clone()));

    let pipelines = Pipelines { camera_manager, cv_manager, errors };
    (config, pipelines)
}
