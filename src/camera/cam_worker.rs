use ccap::{FrameOrientation, PropertyName, Provider};

use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::SharedFrame;
use crate::camera::RgbaBuffer;
use crate::config::CameraConfig;
use crate::utils::ServiceCore;

use super::Frame;

pub struct CameraWorker {
    pub config: CameraConfig,
    pub core: ServiceCore<Frame>,
    pub shared: SharedFrame,
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

        let req_width = camera.get_property(PropertyName::Width)? as u32;
        let req_height = camera.get_property(PropertyName::Height)? as u32;
        println!("Camera started: {device} @ {req_width}x{req_height} (requested)");

        let pool: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));

        thread::spawn(move || {
            while self.core.is_running() {
                match camera.grab_frame(3000) {
                    Ok(Some(frame)) => {
                        // Trust the frame's *own* reported geometry rather than the
                        // provider's negotiated Width/Height: on macOS the FaceTime
                        // camera silently ignores set_resolution and delivers a
                        // different size (e.g. 1280x720) than get_property reports
                        // (640x480). Sizing the buffer from the stale request made
                        // copy_from_slice panic on the first frame. Reading width,
                        // height and stride per frame also handles row padding that
                        // AVFoundation/Media Foundation add for alignment.
                        let info = match frame.info() {
                            Ok(info) => info,
                            Err(e) => {
                                eprintln!("Failed to read frame info: {e}");
                                thread::sleep(Duration::from_millis(100));
                                continue;
                            }
                        };
                        let Some(src) = info.data_planes[0] else {
                            eprintln!("Captured frame had no data plane");
                            thread::sleep(Duration::from_millis(100));
                            continue;
                        };
                        let width = info.width;
                        let height = info.height;
                        let src_stride = info.strides[0] as usize;
                        let frame_len = (width * height * 4) as usize;

                        // Windows (DirectShow / Media Foundation) commonly hands
                        // back bottom-up RGBA buffers, which render upside-down
                        // once iced treats the bytes as top-row-first. ccap reports
                        // the row order per frame, so we detect it rather than
                        // hardcoding a #[cfg(windows)] flip: some Windows cameras
                        // already report top-down, and a blanket flip would invert
                        // those. Normalize to upright once here, before the frame
                        // fans out to the UI and CV, so the live feed and the pose
                        // overlay stay in sync and the model sees an upright frame.
                        let orientation = info.orientation;
                        let mut rgba = {
                            let mut pool = pool.lock().unwrap();
                            pool.pop().unwrap_or_else(|| vec![0u8; frame_len])
                        };
                        // Camera geometry is stable within a session, so this only
                        // resizes on the first frame; it also right-sizes any pooled
                        // buffer left over from a previous, differently-sized session.
                        rgba.resize(frame_len, 0);
                        copy_upright(&mut rgba, src, width, height, src_stride, orientation);

                        let buf = RgbaBuffer::pooled(rgba, pool.clone());

                        let captured_frame: Frame = (width, height, Arc::new(buf));

                        // Hand the latest frame to the CV worker (overwriting any
                        // it hasn't taken yet) and fan it out to the UI.
                        self.shared.publish(captured_frame.clone());

                        let _ = self.core.publish(captured_frame);
                    }
                    Ok(None) => {
                        eprintln!("Unable to capture frame");
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        eprintln!("Camera capture failed: {}", e);
                        thread::sleep(Duration::from_millis(100));
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

/// Copy a captured RGBA frame into `dst`, flipping it vertically when the camera
/// reports a bottom-up buffer so the consumer always sees a top-row-first frame.
/// `dst` is tightly packed `width * height * 4` RGBA bytes; `src` may have rows
/// padded to `src_stride` bytes (>= `width * 4`), as AVFoundation and Media
/// Foundation commonly do for alignment, so we copy `width * 4` bytes per row
/// from the strided source rather than assuming a contiguous buffer.
fn copy_upright(
    dst: &mut [u8],
    src: &[u8],
    width: u32,
    height: u32,
    src_stride: usize,
    orientation: FrameOrientation,
) {
    let row = (width * 4) as usize;
    let last = height as usize - 1;
    for (y, dst_row) in dst.chunks_exact_mut(row).enumerate() {
        let src_y = match orientation {
            FrameOrientation::TopToBottom => y,
            FrameOrientation::BottomToTop => last - y,
        };
        let src_start = src_y * src_stride;
        dst_row.copy_from_slice(&src[src_start..src_start + row]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_to_bottom_is_copied_verbatim() {
        // 1x3 RGBA image: three distinct rows, top-down. Must pass through unchanged.
        let src: Vec<u8> = (0..3).flat_map(|r| [r, r, r, 255]).collect();
        let mut dst = vec![0u8; src.len()];
        copy_upright(&mut dst, &src, 1, 3, 4, FrameOrientation::TopToBottom);
        assert_eq!(dst, src);
    }

    #[test]
    fn bottom_to_top_is_flipped_row_wise() {
        // Same 1x3 image but reported bottom-up: rows must reverse, bytes within
        // a row (RGBA channels, left-to-right) must stay put.
        let src: Vec<u8> = (0..3).flat_map(|r| [r, r, r, 255]).collect();
        let mut dst = vec![0u8; src.len()];
        copy_upright(&mut dst, &src, 1, 3, 4, FrameOrientation::BottomToTop);
        let expected: Vec<u8> = (0..3).rev().flat_map(|r| [r, r, r, 255]).collect();
        assert_eq!(dst, expected);
    }

    #[test]
    fn flip_preserves_horizontal_order() {
        // 2x2: a pure vertical flip must not mirror left-right (that would be a
        // 180° rotation). Pixels carry their (x, y) so we can check both axes.
        let px = |x: u8, y: u8| [x, y, 0, 255];
        let src: Vec<u8> = [px(0, 0), px(1, 0), px(0, 1), px(1, 1)]
            .concat();
        let mut dst = vec![0u8; src.len()];
        copy_upright(&mut dst, &src, 2, 2, 8, FrameOrientation::BottomToTop);
        let expected: Vec<u8> = [px(0, 1), px(1, 1), px(0, 0), px(1, 0)].concat();
        assert_eq!(dst, expected);
    }

    #[test]
    fn padded_stride_rows_are_unpadded() {
        // 2x2 image whose source rows are padded to a stride of 12 bytes (8 bytes
        // of pixels + 4 bytes of padding) — mimics AVFoundation/Media Foundation
        // row alignment. The padding must be dropped so `dst` stays tightly packed.
        let px = |x: u8, y: u8| [x, y, 0, 255];
        let pad = [0xEE, 0xEE, 0xEE, 0xEE];
        let src: Vec<u8> = [
            px(0, 0), px(1, 0), pad,
            px(0, 1), px(1, 1), pad,
        ]
        .concat();
        let mut dst = vec![0u8; 2 * 2 * 4];
        copy_upright(&mut dst, &src, 2, 2, 12, FrameOrientation::TopToBottom);
        let expected: Vec<u8> = [px(0, 0), px(1, 0), px(0, 1), px(1, 1)].concat();
        assert_eq!(dst, expected);
    }
}
