# PostureTracker

A cross-platform desktop app (Linux / Windows / macOS) that watches the user's posture
through a webcam and alerts them when they slouch. Goals: accuracy, low resource usage,
and a clean, comfortable UI. Written entirely in Rust.

- **UI**: [`iced`](https://docs.rs/iced) 0.14 (`daemon` multi-window mode)
- **Inference**: [`ort`](https://ort.pyke.io/) 2.0 (ONNX Runtime) running YOLOv8n-pose
- **Capture**: [`ccap`](https://docs.rs/ccap-rs) (cross-platform), with a raw V4L2 ioctl shim on Linux
- **Drawing**: [`raqote`](https://docs.rs/raqote) for the skeleton overlay
- **Persistence**: `toml` config + plain-text daily metric logs

Run with `cargo run`. Tests: `cargo test` (pure-logic unit tests, no camera/GPU needed).

---

## Architecture

Three long-lived pieces connected by channels, driven by iced's Elm-style loop.

```
 Camera thread ──YUYV→RGBA──┐
   (cam_worker)             ├─► SharedFrame (Arc<Mutex<Option<Frame>>>)  ──► CV thread
                            │                                               (cv_worker)
   broadcast::<Frame> ──────┘                                                   │
        │                                                          inference + raqote render
        ▼                                                                       │
   iced subscription ──Message::CamFrame──► App.update            broadcast::<Inference> ──┐
                                                                                            ▼
                                                              iced subscription ──Message::CvInference──► App.update
```

**Camera worker** (`src/camera/cam_worker.rs`): a `std::thread` loop. Grabs a frame from
`ccap`, copies it into a pooled RGBA buffer, then (a) stores a clone in the `SharedFrame`
slot for CV to pick up and (b) broadcasts it to the UI. Buffers are recycled via a
`Vec<Vec<u8>>` pool guarded by `RgbaBuffer`'s `Drop` impl, so steady-state capture does
not reallocate.

**CV worker** (`src/cv/cv_worker.rs`): a `std::thread` loop. `take()`s the latest frame
from the `SharedFrame` slot (dropping any it missed — we only care about the freshest
frame), runs the model, renders the skeleton onto a transparent buffer, and broadcasts an
`Inference { frame, time_metrics, posture_angle_deg }`. Sleeps 5 ms when no frame is
ready.

**App** (`src/app.rs`): the single iced `Application`. Holds all UI state, owns the
`Pipelines` (the two managers), and reacts to `Message`s. `subscription()` bridges the
broadcast channels into iced `Message`s (see `src/app/subscriptions.rs`), plus timers for
background sampling, the calibration countdown, and the metrics slide animation.

### The ManagedService pattern (`src/utils.rs`)

Both the camera and CV pipelines are `Arc<...Manager>` wrappers around a shared
`ServiceCore<T>` = `{ running: Arc<AtomicBool>, tx: broadcast::Sender<T> }`. The
`ManagedService` trait gives them uniform `start` / `stop` / `is_running` / `subscribe`.
`stop()` just flips the atomic; the worker thread notices on its next loop iteration and
exits. `start()` spawns a fresh worker thread. This is why starting/stopping a session is
cheap and why stale in-flight frames are explicitly dropped in `App::update` (a frame can
arrive after `stop()` but before the thread sees the flag).

### Data flow types

- `Frame = (u32, u32, Arc<RgbaBuffer>)` — width, height, pooled RGBA pixels (`src/camera.rs`).
- `Inference` — a rendered overlay frame + `TimeMetrics` + optional posture angle (`src/cv.rs`).
- The model is loaded lazily (`CVManager::load_model`) on first Test/Start, not at boot.

---

## Posture detection

The math is deliberately simple and lives in `src/cv/pose.rs`:

1. YOLOv8n-pose outputs `[1, 56, 8400]`. We pick the single highest-confidence detection
   (`decode_yolo_pose`) — this is a single-user app, so no NMS.
2. Only three COCO keypoints matter: **nose (0)**, **left shoulder (5)**, **right shoulder (6)**
   (`KEEP_KEYPOINTS` in `src/constants.rs`).
3. `posture_angle_deg` = the angle at the nose between the vectors to each shoulder. When
   the user leans toward the screen the shoulders foreshorten and this angle widens; lean
   away and it narrows. It's a proxy, not a true neck angle, but it's stable and cheap.
4. **Calibration** records the user's baseline angle (median of ~5 s of samples). Bad
   posture = `|current − baseline| ≥ threshold_deg` (default 12°).

The full skeleton is still drawn for the live overlay (`draw_skeleton`), but only those
three points drive the alerting.

---

## Run modes & windows

iced runs in `daemon` mode so the app can open/close multiple OS windows:

- **Main / Dashboard window** — always-on-top, fixed size. Live camera + overlay,
  controls, metrics. Has a Settings sub-view (camera picker, thresholds, intervals).
- **Debug window** — opened on demand; raw per-stage timings and live numbers.
- **Alert window** — borderless, maximized, always-on-top. Shown when posture is bad.
- **System tray** — restore/quit when minimized (`src/app/tray.rs`). Gracefully disabled
  if the appindicator library is missing on Linux.

`RunMode` (Foreground vs Background) and `InferenceState` (Unloaded/Stopped/Running) in
`src/app.rs` gate the pipeline. Two sampling strategies in background mode:

- **Continuous** (`interval_secs == 0`): pipeline runs nonstop; alert fires immediately
  on bad posture and auto-dismisses when corrected (unless `force_dismiss`).
- **Interval** (e.g. every 60 s): pipeline is *stopped* between checks to save power; a
  timer wakes it, it collects `frames_per_sample` frames, alerts if a majority are bad,
  then stops again.

An `alert_cooldown` (floor `MIN_ALERT_COOLDOWN_SECS`) prevents the popup from
re-triggering immediately after dismissal.

---

## Metrics (`src/metrics.rs`)

`MetricsStore` tracks Daily / Session / All-Time posture stats (breaks, bad-posture time,
tracked time, quality %, streaks). Durations are accumulated lazily from `Instant`s.

Persistence is **append-only event logs**, one file per day (`<data_dir>/YYYY-MM-DD.log`,
lines like `1700000000000,GoodToBad`). On startup the store replays today's log to restore
counters, folds finished past days into `all_time.toml`, and prunes logs older than
`history_days_to_keep`. This survives crashes and makes the daily rollover idempotent.
Counters only advance during an active tracking session — Testing/Calibrating record angle
samples for the chart but don't pollute stats.

---

## Linux camera gotcha (important)

`ccap` can't decode many webcams' default MJPEG stream — you get green static / colored
bars. `ccap` also won't change the device's pixel format. The fix (`src/camera.rs`,
`v4l2` module): before `ccap` opens the device, we issue a raw `VIDIOC_S_FMT` ioctl to
force **YUYV** at 640×480, which `ccap` then converts to RGBA cleanly. We re-apply this on
every start because cameras reset to MJPEG across reboots. Off Linux this is a no-op
(DirectShow/AVFoundation negotiate format themselves).

Camera *enumeration* is also custom on Linux: we scan `/dev/video*` and `VIDIOC_QUERYCAP`
each node, keeping only capture-capable ones (a single webcam exposes several nodes). Off
Linux we use `ccap`'s name-based device list. See `list_cameras()`.

---

## Module map

| Path | Responsibility |
|------|----------------|
| `src/main.rs` / `src/lib.rs` | Entry point; wires up `Config` + `Pipelines`. |
| `src/app.rs` | The iced app: state, `Message`, `update`, `view`, windows, run-mode logic. |
| `src/app/subscriptions.rs` | Bridges broadcast channels → iced `Message`s. |
| `src/app/tray.rs` | System tray icon + menu. |
| `src/app/theme.rs` | **The only color palette** — "Refined Slate" design tokens. |
| `src/app/components/` | One file per UI panel (camera, control, metrics, status, settings, debug, alert) + shared `ui.rs` kit. |
| `src/camera.rs` + `src/camera/` | Capture pipeline, V4L2 shim, camera enumeration. |
| `src/cv.rs` + `src/cv/` | Inference pipeline: `cv_inference.rs` (ort session), `pose.rs` (decode/render/angle). |
| `src/metrics.rs` | Posture stats + append-only log persistence. |
| `src/config.rs` | `config.toml` schema (serde) at the OS config dir. |
| `src/constants.rs` | Model bytes (embedded via `include_bytes!`), COCO skeleton, thresholds. |
| `src/utils.rs` | `ManagedService` trait + `ServiceCore`. |

Config lives at `~/.config/posturetracker/config.toml`; metric data at
`~/.local/share/posturetracker/` (per `dirs`).

---

## Conventions for future work

- **Colors**: use only the tokens in `src/app/theme.rs`. The aesthetic is a minimal,
  comfortable dark theme (Sublime-Text-like). Build UI from the shared `ui.rs` kit so all
  panels share one button system, surface set, and type scale (uppercase micro-labels,
  monospace numbers).
- **Code style**: idiomatic, consistent Rust. Avoid `.clone()` where a reference or move
  works — frames are large. Prefer `let ... else`, iterator combinators, and syntax sugar
  over verbose forms. Keep modules small and single-purpose (note the per-component and
  per-pipeline file split).
- **Threading**: the workers are plain `std::thread` loops polling an `AtomicBool`; don't
  add async inside them. Async lives only at the iced subscription boundary.
- **Tests**: logic is unit-tested without hardware (see `pose.rs`, `metrics.rs`,
  `config.rs`, `utils.rs`, `app.rs` test modules, and `tests/config_integration.rs`).
  Keep new logic testable in the same hardware-free way.

## References

- iced 0.14 docs: https://docs.rs/iced/0.14 — multi-window `daemon`, `Subscription`,
  `advanced::subscription::Recipe` (used to wrap the broadcast channels).
- ort (ONNX Runtime) book: https://ort.pyke.io/ — `Session`, `inputs!`, `TensorRef`.
- ccap-rs: https://docs.rs/ccap-rs — `Provider`, `PixelFormat`, low-level `sys` bindings.
- YOLOv8 pose output format (Ultralytics): keypoint layout `[x,y,conf] × 17`, COCO order.
  Keypoint index legend is in `src/constants.rs`.
- raqote: https://docs.rs/raqote — `DrawTarget`, `PathBuilder`.
- Prior experiments (kept for context, not used at runtime):
  [rust-webcam-model-bench](https://github.com/schniebly-scott/rust-webcam-model-bench),
  [PostureTracker_PoC](https://github.com/schniebly-scott/PostureTracker_PoC).

## CI / packaging

`.github/workflows/rust.yml` builds release artifacts on `v*.*.*` tags (or manual
dispatch) for Linux (`.deb` + tarball), Windows (`.zip`), and macOS (`.app` bundle), then
drafts a GitHub release. Linux builds need the GTK/X11/Wayland `-dev` packages listed in
the workflow.
