use std::sync::atomic::Ordering;
use std::error::Error;
use std::sync::{Arc, Mutex};

use crate::SharedFrame;
use crate::config::CameraConfig;
use crate::utils::{ManagedService, ServiceCore};

use super::{ Frame, cam_worker::CameraWorker };

#[derive(Debug)]
pub struct RgbaBuffer {
    pub data: Vec<u8>,
    pub pool: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl Drop for RgbaBuffer {
    fn drop(&mut self) {
        let mut pool = self.pool.lock().unwrap();
        pool.push(std::mem::take(&mut self.data));
    }
}

#[derive(Debug)]
pub struct CameraManager {
    config: CameraConfig,
    core: ServiceCore<Frame>,
    shared: SharedFrame,
}

impl CameraManager {
    pub fn new(config: CameraConfig, shared: SharedFrame) -> Self {
        Self {
            config,
            shared,
            core: ServiceCore::new(2)
        }
    } 
}

impl ManagedService for CameraManager {
    type Output = Frame;

    fn core(&self) -> &ServiceCore<Self::Output> {
        &self.core
    }

    fn start(&self) -> Result<(), Box<dyn Error>> {
        self.core.running.store(true, Ordering::SeqCst);

        CameraWorker {
            config: self.config.clone(),
            core: self.core.clone(),
            shared: self.shared.clone(),
        }
        .spawn()
    }
}