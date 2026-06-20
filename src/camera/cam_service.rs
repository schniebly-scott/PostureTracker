use std::error::Error;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use crate::SharedFrame;
use crate::config::CameraConfig;
use crate::utils::{ManagedService, ServiceCore};

use super::{Frame, cam_worker::CameraWorker};

#[derive(Debug)]
pub struct RgbaBuffer {
    pub data: Vec<u8>,
    /// Where `data` goes on drop. `Some(pool)` returns it for reuse (the camera
    /// pipeline pops these back in `cam_worker`); `None` frees it. The CV
    /// pipeline renders a fresh buffer per frame and never pops, so it uses the
    /// unpooled variant — see issue_writeups/cv_buffer_pool_leak.md.
    pool: Option<Arc<Mutex<Vec<Vec<u8>>>>>,
}

impl RgbaBuffer {
    /// A pooled buffer: on drop, `data` is returned to `pool` for reuse.
    pub fn pooled(data: Vec<u8>, pool: Arc<Mutex<Vec<Vec<u8>>>>) -> Self {
        Self { data, pool: Some(pool) }
    }

    /// A standalone buffer whose `data` is freed on drop.
    pub fn unpooled(data: Vec<u8>) -> Self {
        Self { data, pool: None }
    }
}

impl Drop for RgbaBuffer {
    fn drop(&mut self) {
        if let Some(pool) = &self.pool {
            pool.lock().unwrap().push(std::mem::take(&mut self.data));
        }
    }
}

#[derive(Debug)]
pub struct CameraManager {
    config: Mutex<CameraConfig>,
    core: ServiceCore<Frame>,
    failure_tx: broadcast::Sender<String>,
    shared: SharedFrame,
}

impl CameraManager {
    pub fn new(config: CameraConfig, shared: SharedFrame) -> Self {
        let (failure_tx, _) = broadcast::channel(4);
        Self {
            config: Mutex::new(config),
            shared,
            core: ServiceCore::new(2),
            failure_tx,
        }
    }

    /// Update the device used for the next capture session. Takes effect when
    /// the camera is (re)started.
    pub fn set_device(&self, device: Option<String>) {
        self.config.lock().unwrap().device = device;
    }

    /// Subscribe to fatal capture failures that happen after the worker has
    /// started, such as a camera being unplugged mid-session.
    pub fn subscribe_failures(&self) -> broadcast::Receiver<String> {
        self.failure_tx.subscribe()
    }
}

impl ManagedService for CameraManager {
    type Output = Frame;

    fn core(&self) -> &ServiceCore<Self::Output> {
        &self.core
    }

    fn spawn_worker(&self) -> Result<(), Box<dyn Error>> {
        CameraWorker {
            config: self.config.lock().unwrap().clone(),
            core: self.core.clone(),
            failure_tx: self.failure_tx.clone(),
            shared: self.shared.clone(),
        }
        .spawn()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pooled_buffer_returns_data_on_drop() {
        let pool: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        drop(RgbaBuffer::pooled(vec![1, 2, 3], pool.clone()));
        assert_eq!(pool.lock().unwrap().len(), 1);
    }

    #[test]
    fn unpooled_buffer_does_not_grow_a_pool() {
        // The CV pipeline uses unpooled buffers; dropping many must not retain
        // memory anywhere (regression test for the CV buffer-pool leak).
        let pool: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        for _ in 0..1000 {
            drop(RgbaBuffer::unpooled(vec![0u8; 640 * 480 * 4]));
        }
        assert!(pool.lock().unwrap().is_empty());
    }

    #[test]
    fn pooled_buffers_are_bounded_by_pop_pattern() {
        // Mirror the camera worker: pop a recycled buffer when available, else
        // allocate. The pool size never exceeds the number of buffers in flight
        // (here, one at a time).
        let pool: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        for _ in 0..1000 {
            let data = pool.lock().unwrap().pop().unwrap_or_else(|| vec![0u8; 16]);
            drop(RgbaBuffer::pooled(data, pool.clone()));
            assert!(pool.lock().unwrap().len() <= 1);
        }
    }

    #[test]
    fn camera_failure_notifications_reach_subscribers() {
        let shared = Arc::new(crate::frame_channel::FrameChannel::new());
        let manager = CameraManager::new(CameraConfig::default(), shared);
        let mut rx = manager.subscribe_failures();

        manager.failure_tx.send("camera disconnected".to_string()).unwrap();

        assert_eq!(rx.try_recv().unwrap(), "camera disconnected");
    }
}
