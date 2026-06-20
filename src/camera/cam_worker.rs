use ccap::{PropertyName, Provider};

use std::error::Error;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::sync::broadcast;

use crate::SharedFrame;
use crate::camera::RgbaBuffer;
use crate::config::CameraConfig;
use crate::utils::ServiceCore;

use super::Frame;

const MAX_CONSECUTIVE_CAPTURE_FAILURES: usize = 5;
const CAPTURE_RETRY_DELAY: Duration = Duration::from_millis(100);

#[derive(Default)]
struct ConsecutiveFailures {
    count: usize,
}

impl ConsecutiveFailures {
    fn record_failure(&mut self) -> bool {
        self.count += 1;
        self.count >= MAX_CONSECUTIVE_CAPTURE_FAILURES
    }

    fn record_success(&mut self) {
        self.count = 0;
    }
}

pub struct CameraWorker {
    pub config: CameraConfig,
    pub core: ServiceCore<Frame>,
    pub failure_tx: broadcast::Sender<String>,
    pub shared: SharedFrame,
}

fn handle_capture_failure(
    failures: &mut ConsecutiveFailures,
    detail: String,
    core: &ServiceCore<Frame>,
    failure_tx: &broadcast::Sender<String>,
) -> bool {
    // A stop can race with a pending grab. In that case this is an intentional
    // shutdown, not a camera failure that should be shown to the user.
    if !core.running.load(Ordering::SeqCst) {
        return true;
    }

    eprintln!("Camera capture failed: {detail}");
    if !failures.record_failure() {
        thread::sleep(CAPTURE_RETRY_DELAY);
        return false;
    }

    let message = format!(
        "Camera stopped after {MAX_CONSECUTIVE_CAPTURE_FAILURES} consecutive capture failures: \
         {detail}. Check that it is connected and allowed in system privacy settings, then \
         select it again in Settings."
    );
    eprintln!("{message}");
    core.running.store(false, Ordering::SeqCst);
    let _ = failure_tx.send(message);
    true
}

impl CameraWorker {
    pub fn spawn(self) -> Result<(), Box<dyn Error>> {
        let device = self
            .config
            .device
            .as_deref()
            .ok_or("No camera device configured")?;

        // Linux only: ccap can't decode the camera's default MJPEG (it renders
        // as green static / colored bars) and won't switch the device format
        // itself. Put the device into raw YUYV at the capture resolution first;
        // ccap then captures and converts it to RGBA cleanly. Webcams commonly
        // reset to MJPEG across reboots, so we do this every time rather than
        // relying on the device default. On Windows/macOS this is a no-op.
        if !crate::camera::set_capture_format(device) {
            eprintln!("Warning: could not set {device} to YUYV; capture may be corrupted");
        }

        let mut camera = Provider::with_device_name(device)?;
        camera.set_pixel_format(ccap::PixelFormat::Rgba32)?;

        // Off Linux the V4L2 ioctl above didn't pick the resolution, so request
        // it from ccap. Best-effort: we read the actual size back below either
        // way, so a camera that rejects it just runs at its native resolution.
        #[cfg(not(target_os = "linux"))]
        if let Err(e) =
            camera.set_resolution(crate::camera::CAPTURE_WIDTH, crate::camera::CAPTURE_HEIGHT)
        {
            eprintln!("Warning: could not set capture resolution: {e}");
        }

        let width = camera.get_property(PropertyName::Width)? as u32;
        let height = camera.get_property(PropertyName::Height)? as u32;
        println!("Camera started: {device} @ {width}x{height}");

        let pool: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));

        thread::spawn(move || {
            let frame_len = (width * height * 4) as usize;
            let mut failures = ConsecutiveFailures::default();

            while self.core.running.load(Ordering::SeqCst) {
                match camera.grab_frame(3000) {
                    Ok(Some(frame)) => {
                        let data = match frame.data() {
                            Ok(data) => data,
                            Err(e) => {
                                if handle_capture_failure(
                                    &mut failures,
                                    format!("unable to read frame data: {e}"),
                                    &self.core,
                                    &self.failure_tx,
                                ) {
                                    break;
                                }
                                continue;
                            }
                        };
                        if data.len() != frame_len {
                            if handle_capture_failure(
                                &mut failures,
                                format!(
                                    "received {} bytes for a {frame_len}-byte RGBA frame",
                                    data.len()
                                ),
                                &self.core,
                                &self.failure_tx,
                            ) {
                                break;
                            }
                            continue;
                        }
                        failures.record_success();

                        let mut rgba = {
                            let mut pool = pool.lock().unwrap();
                            pool.pop().unwrap_or_else(|| vec![0u8; frame_len])
                        };
                        rgba.copy_from_slice(data);

                        let buf = RgbaBuffer::pooled(rgba, pool.clone());

                        let captured_frame: Frame = (width, height, Arc::new(buf));

                        // Hand the latest frame to the CV worker (overwriting any
                        // it hasn't taken yet) and fan it out to the UI.
                        self.shared.publish(captured_frame.clone());

                        let _ = self.core.tx.send(captured_frame);
                    }
                    Ok(None) => {
                        if handle_capture_failure(
                            &mut failures,
                            "capture timed out without a frame".to_string(),
                            &self.core,
                            &self.failure_tx,
                        ) {
                            break;
                        }
                    }
                    Err(e) => {
                        if handle_capture_failure(
                            &mut failures,
                            e.to_string(),
                            &self.core,
                            &self.failure_tx,
                        ) {
                            break;
                        }
                    }
                }
            }

            // No more frames will arrive; wake the CV worker so it observes the
            // stop and leaves its blocking wait instead of parking forever.
            self.shared.wake();
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_failures_stop_at_the_limit() {
        let mut failures = ConsecutiveFailures::default();

        for _ in 1..MAX_CONSECUTIVE_CAPTURE_FAILURES {
            assert!(!failures.record_failure());
        }
        assert!(failures.record_failure());
    }

    #[test]
    fn successful_frame_resets_the_failure_streak() {
        let mut failures = ConsecutiveFailures::default();

        for _ in 1..MAX_CONSECUTIVE_CAPTURE_FAILURES {
            assert!(!failures.record_failure());
        }
        failures.record_success();

        assert!(!failures.record_failure());
    }
}
