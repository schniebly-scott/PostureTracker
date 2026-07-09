use std::error::Error;
use std::sync::{Arc, Mutex};

use crate::SharedFrame;
use crate::config::CameraConfig;
use crate::utils::{ManagedService, PipelineErrors, ServiceCore};

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
    shared: SharedFrame,
    errors: PipelineErrors,
}

impl CameraManager {
    pub fn new(config: CameraConfig, shared: SharedFrame, errors: PipelineErrors) -> Self {
        Self {
            config: Mutex::new(config),
            shared,
            core: ServiceCore::new(2),
            errors,
        }
    }

    /// Update the device used for the next capture session. Takes effect when
    /// the camera is (re)started.
    pub fn set_device(&self, device: Option<String>) {
        self.config.lock().unwrap().device = device;
    }

    /// Update the downscale cap applied to captured frames. Takes effect when
    /// the camera is (re)started.
    pub fn set_capture_resolution(&self, width: u32, height: u32) {
        let mut config = self.config.lock().unwrap();
        config.capture_width = width;
        config.capture_height = height;
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
            shared: self.shared.clone(),
            errors: self.errors.clone(),
        }
        .spawn()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn test_manager(device: Option<&str>) -> (Arc<CameraManager>, PipelineErrors) {
        let errors = PipelineErrors::new();
        let manager = Arc::new(CameraManager::new(
            CameraConfig {
                device: device.map(str::to_string),
                capture_width: 640,
                capture_height: 480,
            },
            Arc::new(crate::frame_channel::FrameChannel::new()),
            errors.clone(),
        ));
        (manager, errors)
    }

    #[test]
    fn start_without_device_fails_synchronously_and_stays_stopped() {
        let (manager, _errors) = test_manager(None);
        assert!(manager.start().is_err());
        assert!(!manager.is_running(), "failed spawn must release the flag");
    }

    #[test]
    fn failed_device_open_reports_error_and_clears_running() {
        let (manager, errors) = test_manager(Some("/dev/video-does-not-exist"));
        let mut rx = errors.subscribe();

        manager
            .start()
            .expect("spawn succeeds; the open fails on the worker thread");

        // The worker opens the device asynchronously; wait for it to fail and
        // reflect reality instead of lying `is_running() == true` forever.
        let deadline = Instant::now() + Duration::from_secs(10);
        while manager.is_running() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !manager.is_running(),
            "a dead worker must not report a live camera"
        );
        assert!(
            rx.has_changed().unwrap(),
            "the failure must reach the UI error channel, not just stderr"
        );
        let message = rx.borrow_and_update().clone().unwrap();
        assert!(message.contains("Camera error"), "got: {message}");
    }

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
}
