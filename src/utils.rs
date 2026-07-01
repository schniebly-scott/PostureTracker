use std::{
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
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
    session: Arc<Mutex<Session>>,
    tx: broadcast::Sender<T>,
}

/// Per-session run state. The flag is a *fresh* `Arc<AtomicBool>` for every
/// session (see `claim_running`): a worker snapshots its own session's flag at
/// spawn and loops on that, never on the core's current flag. This is what
/// makes an immediate stop→start restart safe — the old worker's flag stays
/// false forever even though the service as a whole is running again, so the
/// old thread always exits instead of adopting the new session (previously
/// every camera/resolution switch leaked the old capture thread, which never
/// observed the stop; see issue_writeups/worker_thread_leak_on_restart.md).
#[derive(Debug)]
struct Session {
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl<T: Clone> ServiceCore<T> {
    pub fn new(buffer: usize) -> Self {
        let (tx, _) = broadcast::channel(buffer);
        Self {
            session: Arc::new(Mutex::new(Session {
                running: Arc::new(AtomicBool::new(false)),
                worker: None,
            })),
            tx,
        }
    }

    /// Claim the running flag for a new session. Replaces the session flag with
    /// a fresh one so any straggling worker from a previous session (still
    /// holding the old flag) can never mistake the new session for its own.
    pub fn claim_running(&self) -> bool {
        let mut session = self.session.lock().unwrap();
        if session.running.load(Ordering::SeqCst) {
            return false;
        }
        session.running = Arc::new(AtomicBool::new(true));
        true
    }

    pub fn mark_running(&self, running: bool) {
        self.session
            .lock()
            .unwrap()
            .running
            .store(running, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.session.lock().unwrap().running.load(Ordering::SeqCst)
    }

    /// The current session's run flag. Workers capture this once at spawn and
    /// loop on it; they must not call `is_running`, which reads whatever
    /// session is current at the time.
    pub fn session_flag(&self) -> Arc<AtomicBool> {
        self.session.lock().unwrap().running.clone()
    }

    /// Record the just-spawned worker's handle so the next session can join it.
    pub fn adopt_worker(&self, handle: JoinHandle<()>) {
        self.session.lock().unwrap().worker = Some(handle);
    }

    /// Take the previous session's worker handle (if any) so the caller can
    /// join it before starting a replacement. The lock is released before any
    /// join happens, so a draining worker can't deadlock against the core.
    pub fn take_prev_worker(&self) -> Option<JoinHandle<()>> {
        self.session.lock().unwrap().worker.take()
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
    fn restart_does_not_rearm_previous_sessions_flag() {
        // The thread-leak regression: a worker from session 1 still holds its
        // session flag across stop→start (it was blocked in grab_frame). The
        // restart must hand session 2 a *new* flag and leave session 1's flag
        // false, so the old worker exits instead of adopting the new session.
        let core = ServiceCore::<u32>::new(4);

        assert!(core.claim_running());
        let old_flag = core.session_flag();

        core.mark_running(false); // stop()
        assert!(core.claim_running()); // immediate start()

        assert!(!old_flag.load(Ordering::SeqCst), "old worker must see stop");
        assert!(core.is_running(), "new session is live");
        // The new session's flag is a distinct object.
        assert!(!Arc::ptr_eq(&old_flag, &core.session_flag()));
    }

    #[test]
    fn adopted_worker_handle_is_taken_once() {
        let core = ServiceCore::<u32>::new(4);
        core.adopt_worker(std::thread::spawn(|| {}));

        let prev = core.take_prev_worker().expect("handle was adopted");
        prev.join().unwrap();
        assert!(core.take_prev_worker().is_none());
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
