pub mod app;
pub mod camera;
pub mod config;
pub mod constants;
pub mod cv;
pub mod metrics;
pub mod utils;

use std::sync::{Arc, Mutex};

use camera::{CameraManager, Frame};
use config::Config;
use cv::CVManager;

pub use app::run;

pub type SharedFrame = Arc<Mutex<Option<Frame>>>;

#[derive(Clone, Debug)]
pub struct Pipelines {
    pub camera_manager: Arc<CameraManager>,
    pub cv_manager: Arc<CVManager>,
}

pub fn new_app_state() -> (Config, Pipelines) {
    let config = Config::load_or_default(config::config_path());

    let shared_frame: SharedFrame = Arc::new(Mutex::new(None));
    let camera_manager = Arc::new(CameraManager::new(config.camera.clone(), shared_frame.clone()));
    let cv_manager = Arc::new(CVManager::new(shared_frame.clone()));

    let pipelines = Pipelines { camera_manager, cv_manager };
    (config, pipelines)
}
