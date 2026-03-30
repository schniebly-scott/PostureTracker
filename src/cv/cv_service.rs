use std::sync::atomic::Ordering;
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

    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.core.running.store(true, Ordering::SeqCst);

        CVWorker {
            model: self.model.clone(),
            shared: self.shared.clone(),
            core: self.core.clone(),
        }
        .spawn()
    }
}
