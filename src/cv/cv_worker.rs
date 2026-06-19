use std::sync::Arc;
use std::sync::Mutex;
use std::{error::Error, thread};

use super::{Inference, cv_inference::Model};
use crate::SharedFrame;
use crate::camera::RgbaBuffer;
use crate::utils::ServiceCore;

pub struct CVWorker {
    pub model: Arc<Mutex<Option<Model>>>,
    pub shared: SharedFrame,
    pub core: ServiceCore<Inference>,
}

impl CVWorker {
    pub fn spawn(self) -> Result<(), Box<dyn Error>> {
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

            // Block until a frame is published or the pipeline is stopped. No
            // polling: an idle pipeline parks here with zero wakeups, and a
            // stop wakes us with `None` so the loop exits.
            while let Some(frame) = self.shared.wait(&self.core.running) {
                // ---------- Extract RGBA ----------
                let (width, height, rgba) = (frame.0, frame.1, frame.2.data.clone());

                // ---------- Inference ----------
                let (output, time_metrics, posture_angle_deg) =
                    match model.process_rgba(&rgba, width, height) {
                        Ok(o) => o,
                        Err(e) => {
                            eprintln!("Inference error: {e}");
                            continue;
                        }
                    };

                // ---------- Publish result ----------
                // Not pooled: the overlay is freshly rendered each frame and
                // never recycled, so returning it to a pool only grows that
                // pool unboundedly (issue_writeups/cv_buffer_pool_leak.md).
                let buf = RgbaBuffer::unpooled(output);

                let _ = self.core.tx.send(Inference {
                    frame: (width, height, Arc::new(buf)),
                    time_metrics,
                    posture_angle_deg,
                });
            }
        });

        Ok(())
    }
}
