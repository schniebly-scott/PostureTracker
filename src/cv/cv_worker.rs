use std::sync::atomic::Ordering;
use std::time::Duration;
use std::{thread, time::Instant, error::Error};
use std::sync::{Arc, Mutex};

use crate::camera::RgbaBuffer;
use crate::SharedFrame;
use crate::utils::ServiceCore;
use super::{Inference, cv_inference::Model};

pub struct CVWorker {
    pub model: Arc<Mutex<Option<Model>>>,
    pub shared: SharedFrame,
    pub core: ServiceCore<Inference>
}

impl CVWorker {
    pub fn spawn(self) -> Result<(), Box<dyn Error>> {
        let pool: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));

        thread::spawn(move || {
            // ---------- Get reference to Model inside thread ----------
            let mut model_lock = self.model.lock().unwrap();

            let model = match model_lock.as_mut() {
                Some(p) => p,
                None => {
                    eprintln!("Model not loaded!");
                    return;
                }
            };

            while self.core.running.load(Ordering::SeqCst) {
                let frame_opt = {
                    let mut slot = self.shared.lock().unwrap();
                    slot.take() // take() = replace with None
                };

                if let Some(frame) = frame_opt {
                    // ---------- Extract RGBA ----------
                    let (width, height, rgba) = (frame.0, frame.1, frame.2.data.clone());

                    // ---------- Inference ----------
                    let now = Instant::now();
                    let output = match model.process_rgba(&rgba, width, height) {
                        Ok(o) => o,
                        Err(e) => {
                            eprintln!("Inference error: {e}");
                            continue;
                        }
                    };
                    let elapsed = now.elapsed();

                    // ---------- Publish result ----------
                    let buf = RgbaBuffer {
                        data: output,
                        pool: pool.clone(),
                    };

                    let _ = self.core.tx.send(Inference { frame: (width, height, Arc::new(buf)), inf_time: elapsed });
                } else {
                    //No frame available, yield CPU
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        });

        Ok(())
    } 
}