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

    /// Spawn the background worker. Called by `start` only after the running
    /// flag has been claimed; implementors must not toggle `running`
    /// themselves.
    fn spawn_worker(&self) -> Result<(), Box<dyn Error>>;

    /// Start the service. Claims the running flag atomically and rejects a
    /// second start while a worker is already live — this closes the
    /// double-spawn race for every `ManagedService` in one place (see
    /// issue_writeups/cv_worker_model_mutex.md).
    fn start(&self) -> Result<(), Box<dyn Error>> {
        if !self.core().claim_running() {
            return Err("service is already running".into());
        }

        // Clear the flag if spawning fails so the next start can retry.
        if let Err(e) = self.spawn_worker() {
            self.core().mark_running(false);
            return Err(e);
        }

        Ok(())
    }

    fn is_running(&self) -> bool {
        self.core().is_running()
    }

    fn stop(&self) {
        self.core().mark_running(false);
    }

    fn subscribe(&self) -> broadcast::Receiver<Self::Output> {
        self.core().subscribe()
    }
}

#[derive(Debug, Clone)]
pub struct ServiceCore<T: Clone> {
    running: Arc<AtomicBool>,
    tx: broadcast::Sender<T>,
}

impl<T: Clone> ServiceCore<T> {
    pub fn new(buffer: usize) -> Self {
        let (tx, _) = broadcast::channel(buffer);
        Self {
            running: Arc::new(AtomicBool::new(false)),
            tx,
        }
    }

    pub fn claim_running(&self) -> bool {
        self.running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn mark_running(&self, running: bool) {
        self.running.store(running, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn running_flag(&self) -> &AtomicBool {
        &self.running
    }

    pub fn publish(&self, value: T) -> Result<usize, broadcast::error::SendError<T>> {
        self.tx.send(value)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<T> {
        self.tx.subscribe()
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

        fn spawn_worker(&self) -> Result<(), Box<dyn Error>> {
            Ok(())
        }
    }

    #[test]
    fn service_core_starts_not_running() {
        let core = ServiceCore::<u32>::new(4);
        assert!(!core.is_running());
    }

    #[test]
    fn broadcast_delivers_to_subscriber() {
        let core = ServiceCore::<u32>::new(4);
        let mut rx = core.subscribe();
        core.publish(42).unwrap();
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
    fn start_rejects_second_start_while_running() {
        let svc = DummyService {
            core: ServiceCore::new(4),
        };

        assert!(svc.start().is_ok());
        // A second start without an intervening stop must be rejected rather
        // than silently spawning an overlapping worker.
        assert!(svc.start().is_err());

        svc.stop();
        // After stopping, the service can be started again.
        assert!(svc.start().is_ok());
    }

    #[test]
    fn managed_service_subscribe_receives_messages() {
        let svc = DummyService {
            core: ServiceCore::new(4),
        };
        let mut rx = svc.subscribe();
        svc.core().publish(7).unwrap();
        assert_eq!(rx.try_recv().unwrap(), 7);
    }
}
