use std::{error::Error, sync::{Arc, atomic::{AtomicBool, Ordering}}};
use tokio::sync::broadcast;

pub trait ManagedService {
    type Output: Clone;

    fn core(&self) -> &ServiceCore<Self::Output>;

    fn start(&self) -> Result<(), Box<dyn Error>>;

    fn stop(&self) {
        self.core().running.store(false, Ordering::SeqCst);
    }

    fn subscribe(&self) -> broadcast::Receiver<Self::Output> {
        self.core().tx.subscribe()
    }
}

#[derive(Debug, Clone)]
pub struct ServiceCore<T: Clone> {
    pub running: Arc<AtomicBool>,
    pub tx: broadcast::Sender<T>,
}

impl<T: Clone> ServiceCore<T> {
    pub fn new(buffer: usize) -> Self {
        let (tx, _) = broadcast::channel(buffer);
        Self {
            running: Arc::new(AtomicBool::new(false)),
            tx,
        }
    }
}
