use ccap::{PropertyName, Provider};

use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use std::error::Error;
use std::sync::{Arc, Mutex};

use crate::SharedFrame;
use crate::camera::RgbaBuffer;
use crate::config::CameraConfig;
use crate::utils::ServiceCore;

use super::Frame;

pub struct CameraWorker {
    pub config: CameraConfig,
    pub core: ServiceCore<Frame>,
    pub shared: SharedFrame,
}

impl CameraWorker {
    pub fn spawn(self) -> Result<(), Box<dyn Error>> {
        let mut camera = Provider::with_device_name(&self.config.device)?;
        camera.set_pixel_format(ccap::PixelFormat::Rgba32)?;

        let width = camera.get_property(PropertyName::Width)? as u32;
        let height = camera.get_property(PropertyName::Height)? as u32;
        println!(
            "Camera started successfully, real resolution: {}x{}",
            width, height
        );

        let pool: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));

        thread::spawn(move || {
            let frame_len = (width * height * 4) as usize;

            while self.core.running.load(Ordering::SeqCst) {
                match camera.grab_frame(3000) {
                    Ok(Some(frame)) => {
                        let data = frame.data().unwrap();
                        let mut rgba = {
                            let mut pool = pool.lock().unwrap();
                            pool.pop().unwrap_or_else(|| vec![0u8; frame_len])
                        };
                        rgba.copy_from_slice(data);

                        let buf = RgbaBuffer {
                            data: rgba,
                            pool: pool.clone(),
                        };

                        let captured_frame: Frame =
                            (width, height, Arc::new(buf));

                        let mut slot = self.shared.lock().unwrap();
                        *slot = Some(captured_frame.clone());

                        let _ = self.core.tx.send(captured_frame);
                    }
                    Ok(None) => {
                        eprintln!("Unable to capture frame");
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        eprintln!("Camera capture failed: {}", e);
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        });
        Ok(())
    }
}