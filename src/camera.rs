mod cam_service;
mod cam_worker;

use std::sync::Arc;

pub use cam_service::{CameraManager, RgbaBuffer};

/// A selectable camera: a human-readable label plus the `/dev/videoN` path that
/// the capture worker opens.
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

/// Minimal V4L2 `VIDIOC_QUERYCAP` binding used to detect capture-capable nodes
/// and read a device's friendly name without pulling in a v4l crate.
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

    // struct v4l2_format: { __u32 type; <4 bytes pad to 8-byte align>; union fmt[200] }.
    // We only populate the leading `struct v4l2_pix_format` (width, height,
    // pixelformat, field, ...) within the union and let the driver fill the rest.
    #[repr(C)]
    struct V4l2Format {
        type_: u32,
        _pad: u32,
        fmt: [u8; 200],
    }

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
            fmt.fmt[0..4].copy_from_slice(&width.to_ne_bytes());
            fmt.fmt[4..8].copy_from_slice(&height.to_ne_bytes());
            fmt.fmt[8..12].copy_from_slice(&V4L2_PIX_FMT_YUYV.to_ne_bytes());
            fmt.fmt[12..16].copy_from_slice(&V4L2_FIELD_NONE.to_ne_bytes());
            let rc = libc::ioctl(fd, VIDIOC_S_FMT, &mut fmt as *mut _ as *mut c_void);
            libc::close(fd);
            rc == 0
        }
    }
}

/// Capture resolution requested from the camera (YUYV, widely supported at 30fps).
pub const CAPTURE_WIDTH: u32 = 640;
pub const CAPTURE_HEIGHT: u32 = 480;

/// Put the given V4L2 device into YUYV mode before capture. See [`v4l2::set_yuyv`].
pub fn set_capture_format(device: &str) -> bool {
    v4l2::set_yuyv(device, CAPTURE_WIDTH, CAPTURE_HEIGHT)
}

/// RGBA frame sent to the UI
/// (width, height, RgbaBuffer { frame, pool-pointer })
pub type Frame = (u32, u32, Arc<RgbaBuffer>);
