use std::sync::Arc;
use std::sync::Mutex;
use std::{error::Error, thread};

use super::{Inference, cv_inference::Model};
use crate::SharedFrame;
use crate::camera::RgbaBuffer;
use crate::utils::ServiceCore;

pub struct CVWorker {
    /// The model, owned by value for the lifetime of this session. It is taken
    /// out of `slot` by `CVManager::spawn_worker` and returned to `slot` when
    /// the worker exits — so the shared mutex is never held across frames.
    pub model: Model,
    pub slot: Arc<Mutex<Option<Model>>>,
    pub shared: SharedFrame,
    pub core: ServiceCore<Inference>,
}

impl CVWorker {
    pub fn spawn(self) -> Result<(), Box<dyn Error>> {
        thread::spawn(move || {
            let CVWorker { mut model, slot, shared, core } = self;

            // Block until a frame is published or the pipeline is stopped. No
            // polling: an idle pipeline parks here with zero wakeups, and a
            // stop wakes us with `None` so the loop exits.
            while let Some(frame) = shared.wait(&core.running) {
                // ---------- Extract dimensions (borrow pixels directly) ----------
                let (width, height) = (frame.0, frame.1);

                // ---------- Inference ----------
                let (output, time_metrics, posture_angle_deg) = match model.process_rgba(&frame.2.data, width, height)
                {
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

                let _ = core.tx.send(Inference {
                    frame: (width, height, Arc::new(buf)),
                    time_metrics,
                    posture_angle_deg,
                });
            }

            // Return the model so the next session can reuse it. Don't clobber a
            // model that was (re)loaded into the slot while we were running.
            let mut slot = slot.lock().unwrap();
            if slot.is_none() {
                *slot = Some(model);
            }
        });

        Ok(())
    }
}
