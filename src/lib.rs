pub mod app;
pub mod camera;
pub mod cv;
pub mod config;
pub mod utils;
pub mod constants;

use std::sync::{Arc, Mutex};

use camera::{CameraManager, Frame};
use cv::{CVManager, Inference};
use config::Config;

pub use app::run;

pub type SharedFrame = Arc<Mutex<Option<Frame>>>;

#[derive(Clone, Debug)]
pub struct Pipelines {
    pub camera_manager: Arc<CameraManager>,
    pub cv_manager: Arc<CVManager>
}

pub fn new_pipelines() -> Pipelines {
    let config = Config::load("config.toml").expect("Unable to load config");

    let shared_frame: SharedFrame = Arc::new(Mutex::new(None));

    let camera_manager = Arc::new(CameraManager::new(config.camera, shared_frame.clone()));
    let cv_manager = Arc::new(CVManager::new(shared_frame.clone()));

    Pipelines { camera_manager, cv_manager }
}