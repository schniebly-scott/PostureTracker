# Camera feed is upside-down on Windows

**Severity: medium (Windows-only) — the live feed and the pose overlay both render
inverted, which is disorienting and can confuse calibration ("sit upright" reads as
"sit upside-down"). Pose math is unaffected, but the UX is broken.**

## Where
- `src/camera/cam_worker.rs` ~lines 40–90: frames come straight out of `ccap`
  (`camera.grab_frame(...)` → `frame.data()` → `RgbaBuffer`) and are passed through
  to the UI/CV with no vertical orientation handling.
- `src/app/subscriptions.rs` ~lines 51 & 98: `image::Handle::from_rgba(w, h, data)` —
  the raw bytes are handed to iced as-is, top row first.
- `src/app/components/camera_panel.rs` ~lines 134–160: both the camera image and the
  CV overlay are drawn with `ContentFit::Contain`; whatever row order the buffer has is
  what's shown. (Because the overlay is rendered against the *same* buffer, it inverts
  in lockstep, so the overlay stays aligned to the feed — it's just that both are
  flipped.)

## Why this happens on Windows specifically
`ccap` is a thin wrapper over each OS's native capture API, and those APIs disagree on
row order:
- On Linux (V4L2) and macOS (AVFoundation) the RGBA buffer ccap hands back is
  top-row-first, which is what `image::Handle::from_rgba` expects.
- On Windows, the underlying capture path (DirectShow / Media Foundation backing a
  number of webcams) commonly delivers **bottom-up** bitmaps — the classic
  negative-vs-positive `biHeight` DIB convention. When a bottom-up buffer is fed to a
  top-down consumer with no flip, the image is vertically mirrored (upside-down).

So this is not a camera-hardware quirk; it's the well-known Windows bitmap row-order
convention leaking through ccap. The capture-format shim we already have
(`set_capture_format`, Linux-only, see `cam_worker.rs` ~line 36 and the
[ccap-mjpeg-yuyv-gotcha] memory) is the analogous "ccap doesn't normalize the platform
for us" problem on the format axis — orientation is the same class of issue on the
geometry axis.

## Things to confirm before fixing
1. Reproduce and pin the cause: log a known-orientation scene on a Windows machine and
   confirm it's a pure vertical flip (upside-down) and not a 180° rotation (which would
   also mirror left-right). DirectShow bottom-up is a vertical flip only.
2. Check whether it's camera-dependent. Some Windows webcams report top-down; if so a
   blanket flip would *break* those. Prefer detecting the orientation from the capture
   API (DIB `biHeight` sign / MF stride sign) over a hardcoded `#[cfg(windows)]` flip.
3. Verify ccap doesn't already expose an orientation/flip property we can set at open
   time (cheaper and correct-by-construction than flipping bytes ourselves).

## Suggested direction
- Preferred: set an orientation/flip option on the ccap provider at open time if one
  exists, alongside the existing pixel-format/resolution setup in `cam_worker.rs`.
- If ccap can't do it, do a vertical row-flip in the capture thread when the platform
  reports bottom-up. The buffer is already `width*height*4` contiguous RGBA in
  `cam_worker.rs`; flipping is a row-wise reverse copy. Do it *once* in the capture
  thread (before it fans out to UI + CV) so the feed and the pose overlay stay in sync
  and the CV model sees an upright frame. Reuse the existing pooled buffer rather than
  allocating per frame (cf. the [cv_buffer_pool_leak] / [eliminate_frame_clones] work).
- Gate the flip on a detected condition, not just `target_os = "windows"`, unless
  testing shows every supported Windows path is bottom-up.

## Acceptance check
On Windows, start a session and confirm the live feed is upright, text/objects in the
scene read correctly, and the pose skeleton overlay still lines up with the body. Re-run
on Linux/macOS to confirm no regression (feed still upright there).
