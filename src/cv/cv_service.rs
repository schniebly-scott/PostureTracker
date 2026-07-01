use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{error::Error, time::Instant};

use super::{Inference, cv_inference::Model};
use crate::SharedFrame;
use crate::cv::cv_worker::CVWorker;
use crate::utils::{ManagedService, ServiceCore};

#[derive(Debug)]
pub struct CVManager {
    model: Arc<Mutex<Option<Model>>>,
    shared: SharedFrame,
    core: ServiceCore<Inference>,
}

impl CVManager {
    pub fn new(shared: SharedFrame) -> Self {
        Self {
            model: Arc::new(Mutex::new(None)),
            shared,
            core: ServiceCore::new(1),
        }
    }

    pub fn load_model(&self) -> Result<Duration, Box<dyn Error>> {
        // A live worker owns the model by value (see `spawn_worker`), so the
        // shared slot is empty while it runs. Reloading now would be clobbered
        // when the worker returns its model on exit — reject instead.
        if self.is_running() {
            return Err("cannot load model while the CV worker is running".into());
        }

        let now = Instant::now();
        let estimator = Model::new()?;
        let elapsed = now.elapsed();

        let mut model_lock = self.model.lock().unwrap();
        *model_lock = Some(estimator);

        println!("Loading model took {:?}", elapsed);
        Ok(elapsed)
    }
}

impl ManagedService for CVManager {
    type Output = Inference;

    fn core(&self) -> &ServiceCore<Self::Output> {
        &self.core
    }

    fn spawn_worker(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Let the previous session's worker finish before taking the model: on
        // an immediate stop→start it may not have returned the model to the
        // slot yet, and taking too early would fail with "model is not loaded".
        // The join is bounded to at most one in-flight inference — `stop` has
        // already woken the frame channel, so a parked worker exits promptly.
        if let Some(prev) = self.core.take_prev_worker() {
            let _ = prev.join();
        }

        // Take the model *out* of the shared slot for the run and hand it to the
        // worker by value. This keeps `load_model` from contending with a live
        // worker for the mutex and makes "model in use" explicit: the slot is
        // None while a session runs, and the worker puts the model back on exit.
        let model = self
            .model
            .lock()
            .unwrap()
            .take()
            .ok_or("model is not loaded")?;

        CVWorker {
            model,
            slot: self.model.clone(),
            shared: self.shared.clone(),
            core: self.core.clone(),
        }
        .spawn()
    }

    /// The worker blocks on the shared frame channel, so clearing `running`
    /// isn't enough on its own — wake the channel too, otherwise a worker
    /// waiting on an idle (stopped) camera would never observe the stop.
    fn stop(&self) {
        self.core.mark_running(false);
        self.shared.wake();
    }
}
