use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::broadcast;

pub trait ManagedService {
    type Output: Clone;

    fn core(&self) -> &ServiceCore<Self::Output>;

    fn start(&self) -> Result<(), Box<dyn Error>>;

    fn is_running(&self) -> bool {
        self.core().running.load(Ordering::SeqCst)
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal ManagedService implementation so the trait's default methods can be
    /// exercised with a real ServiceCore (no async runtime required).
    struct DummyService {
        core: ServiceCore<u32>,
    }

    impl ManagedService for DummyService {
        type Output = u32;

        fn core(&self) -> &ServiceCore<Self::Output> {
            &self.core
        }

        fn start(&self) -> Result<(), Box<dyn Error>> {
            self.core.running.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn service_core_starts_not_running() {
        let core = ServiceCore::<u32>::new(4);
        assert!(!core.running.load(Ordering::SeqCst));
    }

    #[test]
    fn broadcast_delivers_to_subscriber() {
        let core = ServiceCore::<u32>::new(4);
        let mut rx = core.tx.subscribe();
        core.tx.send(42).unwrap();
        assert_eq!(rx.try_recv().unwrap(), 42);
    }

    #[test]
    fn managed_service_lifecycle_methods() {
        let svc = DummyService {
            core: ServiceCore::new(4),
        };

        assert!(!svc.is_running());
        svc.start().unwrap();
        assert!(svc.is_running());
        svc.stop();
        assert!(!svc.is_running());
    }

    #[test]
    fn managed_service_subscribe_receives_messages() {
        let svc = DummyService {
            core: ServiceCore::new(4),
        };
        let mut rx = svc.subscribe();
        svc.core().tx.send(7).unwrap();
        assert_eq!(rx.try_recv().unwrap(), 7);
    }
}
