mod cam_service;
mod cam_worker;

use std::sync::Arc;

pub use cam_service::{CameraManager, RgbaBuffer};

/// A selectable camera: a human-readable label plus the identifier that the
/// capture worker opens — a `/dev/videoN` path on Linux, the device name as
/// reported by the OS capture framework on Windows/macOS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraOption {
    pub label: String,
    pub path: String,
}

impl std::fmt::Display for CameraOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// Enumerate capture-capable cameras on the system.
///
/// We can't rely on `ccap`'s name-based device list: a single physical webcam
/// commonly exposes several V4L2 nodes (e.g. a capture node plus a metadata
/// node) that share an identical name, and those names don't round-trip through
/// `Provider::with_device_name`. Instead we scan `/dev/video*`, query each node
/// with the V4L2 `QUERYCAP` ioctl, and keep only the ones that actually support
/// video capture — storing the `/dev/videoN` path, which opens reliably.
#[cfg(target_os = "linux")]
pub fn list_cameras() -> Vec<CameraOption> {
    let mut nodes: Vec<(u32, String)> = match std::fs::read_dir("/dev") {
        Ok(entries) => entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let num = name.strip_prefix("video")?.parse::<u32>().ok()?;
                Some((num, format!("/dev/{name}")))
            })
            .collect(),
        Err(e) => {
            eprintln!("Failed to enumerate /dev for cameras: {e}");
            return Vec::new();
        }
    };
    nodes.sort_by_key(|(num, _)| *num);

    let mut options: Vec<CameraOption> = nodes
        .into_iter()
        .filter_map(|(_, path)| v4l2::capture_label(&path).map(|label| CameraOption { label, path }))
        .collect();

    // Disambiguate any cameras that report the same label.
    for i in 0..options.len() {
        let dup = options
            .iter()
            .filter(|o| o.label == options[i].label)
            .count()
            > 1;
        if dup {
            let node = options[i].path.rsplit('/').next().unwrap_or("").to_string();
            options[i].label = format!("{} ({node})", options[i].label);
        }
    }

    options
}

/// Enumerate capture-capable cameras on the system.
///
/// On Windows (DirectShow) and macOS (AVFoundation) the framework-reported
/// device name is the identifier `Provider::with_device_name` expects, so we
/// can use ccap's enumeration directly. We call the low-level
/// `ccap_provider_find_device_names_list` binding rather than
/// `Provider::get_devices()` because the latter opens every device to query
/// its capabilities — which can fail or steal a camera that's mid-capture.
#[cfg(not(target_os = "linux"))]
pub fn list_cameras() -> Vec<CameraOption> {
    use ccap::sys;

    let mut names: Vec<String> = Vec::new();
    // Safety: create/destroy of an unopened provider plus a query into a
    // correctly-sized, default-initialized out-struct, per the ccap C API.
    unsafe {
        let provider = sys::ccap_provider_create();
        if provider.is_null() {
            eprintln!("Failed to create ccap provider for camera enumeration");
            return Vec::new();
        }
        let mut list = sys::CcapDeviceNamesList::default();
        if sys::ccap_provider_find_device_names_list(provider, &mut list) {
            for i in 0..list.deviceCount {
                let bytes = &list.deviceNames[i];
                let cstr = std::ffi::CStr::from_ptr(bytes.as_ptr());
                let name = cstr.to_string_lossy().trim().to_string();
                if !name.is_empty() {
                    names.push(name);
                }
            }
        }
        sys::ccap_provider_destroy(provider);
    }

    // Disambiguate identical models: ccap opens devices by name, so duplicate
    // labels are kept distinct for the user even though they select the same
    // underlying name.
    let mut options: Vec<CameraOption> = Vec::new();
    for name in names {
        let dup_count = options.iter().filter(|o| o.path == name).count();
        let label = if dup_count > 0 {
            format!("{name} ({})", dup_count + 1)
        } else {
            name.clone()
        };
        options.push(CameraOption { label, path: name });
    }

    options
}

/// Minimal V4L2 `VIDIOC_QUERYCAP` binding used to detect capture-capable nodes
/// and read a device's friendly name without pulling in a v4l crate.
#[cfg(target_os = "linux")]
mod v4l2 {
    use std::ffi::CString;
    use std::os::raw::c_void;

    const V4L2_CAP_VIDEO_CAPTURE: u32 = 0x0000_0001;
    const V4L2_CAP_DEVICE_CAPS: u32 = 0x8000_0000;
    // _IOR('V', 0, struct v4l2_capability) — struct is 104 bytes.
    const VIDIOC_QUERYCAP: libc::c_ulong = 0x8068_5600;

    #[repr(C)]
    struct V4l2Capability {
        driver: [u8; 16],
        card: [u8; 32],
        bus_info: [u8; 32],
        version: u32,
        capabilities: u32,
        device_caps: u32,
        reserved: [u32; 3],
    }

    fn cstr_to_string(bytes: &[u8]) -> String {
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).trim().to_string()
    }

    /// Returns the device's friendly label if `path` supports video capture,
    /// otherwise `None` (e.g. metadata-only nodes).
    pub fn capture_label(path: &str) -> Option<String> {
        let cpath = CString::new(path).ok()?;
        // Safety: standard libc open/ioctl/close on a device node.
        unsafe {
            let fd = libc::open(cpath.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK);
            if fd < 0 {
                return None;
            }
            let mut cap: V4l2Capability = std::mem::zeroed();
            let rc = libc::ioctl(fd, VIDIOC_QUERYCAP, &mut cap as *mut _ as *mut c_void);
            libc::close(fd);
            if rc != 0 {
                return None;
            }
            // Prefer per-node device_caps when advertised, else the union caps.
            let caps = if cap.capabilities & V4L2_CAP_DEVICE_CAPS != 0 {
                cap.device_caps
            } else {
                cap.capabilities
            };
            if caps & V4L2_CAP_VIDEO_CAPTURE == 0 {
                return None;
            }
            let label = cstr_to_string(&cap.card);
            Some(if label.is_empty() { path.to_string() } else { label })
        }
    }

    // _IOWR('V', 5, struct v4l2_format) — struct v4l2_format is 208 bytes.
    const VIDIOC_S_FMT: libc::c_ulong = 0xC0D0_5605;
    const V4L2_BUF_TYPE_VIDEO_CAPTURE: u32 = 1;
    const V4L2_PIX_FMT_YUYV: u32 = 0x5659_5559; // fourcc 'YUYV'
    const V4L2_FIELD_NONE: u32 = 1;

    #[derive(Clone, Copy)]
    #[repr(C)]
    struct V4l2PixFormat {
        width: u32,
        height: u32,
        pixelformat: u32,
        field: u32,
        bytesperline: u32,
        sizeimage: u32,
        colorspace: u32,
        private: u32,
        flags: u32,
        ycbcr_enc: u32,
        quantization: u32,
        xfer_func: u32,
    }

    // The real union contains pointer-bearing members, so it is 8-byte aligned
    // even though the pixel-format member itself only contains u32 fields.
    #[repr(C, align(8))]
    union V4l2FormatData {
        pix: V4l2PixFormat,
        raw: [u8; 200],
    }

    #[repr(C)]
    struct V4l2Format {
        type_: u32,
        fmt: V4l2FormatData,
    }

    const _: () = assert!(std::mem::size_of::<V4l2PixFormat>() == 48);
    const _: () = assert!(std::mem::offset_of!(V4l2Format, fmt) == 8);
    const _: () = assert!(std::mem::size_of::<V4l2Format>() == 208);

    /// Switch the V4L2 device at `path` to YUYV capture at the given resolution.
    /// Returns whether the ioctl succeeded. ccap leaves the device's current
    /// format untouched, so doing this *before* ccap opens the device makes it
    /// capture raw YUYV (which ccap converts cleanly) instead of MJPEG.
    pub fn set_yuyv(path: &str, width: u32, height: u32) -> bool {
        let Ok(cpath) = CString::new(path) else {
            return false;
        };
        // Safety: standard libc open/ioctl/close with a correctly-sized struct.
        unsafe {
            let fd = libc::open(cpath.as_ptr(), libc::O_RDWR);
            if fd < 0 {
                return false;
            }
            let mut fmt: V4l2Format = std::mem::zeroed();
            fmt.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
            fmt.fmt.pix = V4l2PixFormat {
                width,
                height,
                pixelformat: V4L2_PIX_FMT_YUYV,
                field: V4L2_FIELD_NONE,
                bytesperline: 0,
                sizeimage: 0,
                colorspace: 0,
                private: 0,
                flags: 0,
                ycbcr_enc: 0,
                quantization: 0,
                xfer_func: 0,
            };
            let rc = libc::ioctl(fd, VIDIOC_S_FMT, &mut fmt as *mut _ as *mut c_void);
            libc::close(fd);
            rc == 0
        }
    }
}

/// Capture resolution requested from the camera (YUYV, widely supported at 30fps).
pub const CAPTURE_WIDTH: u32 = 640;
pub const CAPTURE_HEIGHT: u32 = 480;

/// A user-selectable cap on the processed/displayed frame size. The camera may
/// deliver more pixels than this (some ignore the requested resolution); frames
/// are downscaled to fit within the cap, preserving aspect ratio. `0` in either
/// field means "native" — no downscale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureResolution {
    pub width: u32,
    pub height: u32,
}

impl CaptureResolution {
    /// Native — capture and display whatever the camera delivers, no downscale.
    pub const NATIVE: Self = Self { width: 0, height: 0 };

    /// Common sizes offered in the settings dropdown, largest first.
    pub const OPTIONS: [Self; 7] = [
        Self::NATIVE,
        Self { width: 1280, height: 720 },
        Self { width: 960, height: 540 },
        Self { width: 848, height: 480 },
        Self { width: 640, height: 480 },
        Self { width: 640, height: 360 },
        Self { width: 320, height: 240 },
    ];

    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// A cap of 0 in either dimension disables downscaling.
    pub fn is_native(self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Output dimensions for a `src_w`x`src_h` frame: the largest size that fits
    /// within this cap while preserving aspect ratio. Never upscales — a frame
    /// already within the cap (or a native cap) is returned unchanged.
    pub fn fit(self, src_w: u32, src_h: u32) -> (u32, u32) {
        if self.is_native() || src_w == 0 || src_h == 0 {
            return (src_w, src_h);
        }
        let scale = (self.width as f64 / src_w as f64)
            .min(self.height as f64 / src_h as f64)
            .min(1.0);
        if scale >= 1.0 {
            return (src_w, src_h);
        }
        let w = ((src_w as f64 * scale).round() as u32).max(1);
        let h = ((src_h as f64 * scale).round() as u32).max(1);
        (w, h)
    }
}

impl std::fmt::Display for CaptureResolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_native() {
            f.write_str("Native (no downscale)")
        } else {
            write!(f, "{}\u{00D7}{}", self.width, self.height)
        }
    }
}

/// Put the given V4L2 device into YUYV mode before capture. See [`v4l2::set_yuyv`].
#[cfg(target_os = "linux")]
pub fn set_capture_format(device: &str) -> bool {
    v4l2::set_yuyv(device, CAPTURE_WIDTH, CAPTURE_HEIGHT)
}

/// No-op off Linux: AVFoundation/DirectShow negotiate the capture format
/// themselves, so the V4L2 MJPEG workaround doesn't apply.
#[cfg(not(target_os = "linux"))]
pub fn set_capture_format(_device: &str) -> bool {
    true
}

/// RGBA frame sent to the UI
/// (width, height, RgbaBuffer { frame, pool-pointer })
pub type Frame = (u32, u32, Arc<RgbaBuffer>);

#[cfg(test)]
mod tests {
    use super::CaptureResolution;

    #[test]
    fn native_cap_never_downscales() {
        assert_eq!(CaptureResolution::NATIVE.fit(1280, 720), (1280, 720));
        // A zero in either field is treated as native.
        assert_eq!(CaptureResolution::new(0, 480).fit(1280, 720), (1280, 720));
    }

    #[test]
    fn fit_preserves_aspect_ratio() {
        // 16:9 source into a 4:3 cap fits by the tighter (width) axis, so the
        // output stays 16:9 (640x360) rather than being squished to 640x480.
        let cap = CaptureResolution::new(640, 480);
        assert_eq!(cap.fit(1280, 720), (640, 360));
    }

    #[test]
    fn fit_never_upscales() {
        // Source already within the cap is returned unchanged.
        let cap = CaptureResolution::new(1280, 720);
        assert_eq!(cap.fit(640, 480), (640, 480));
    }

    #[test]
    fn fit_matches_exact_cap_when_same_aspect() {
        let cap = CaptureResolution::new(320, 240);
        assert_eq!(cap.fit(640, 480), (320, 240));
    }

    #[test]
    fn display_labels_native_and_sizes() {
        assert_eq!(CaptureResolution::NATIVE.to_string(), "Native (no downscale)");
        assert_eq!(CaptureResolution::new(640, 480).to_string(), "640\u{00D7}480");
    }
}
