# Pose render/preprocess: per-frame allocations and a byte-pushing pixel loop

**Severity: low-medium — steady-state inference cost; also one model-accuracy note
(aspect distortion).**

## Where
`src/cv/pose.rs`:
- `render_pose` (~lines 72–88)
- `preprocess_rgba` (~lines 168–201)
- `decode_yolo_pose` (~lines 27–70)

## Issues

1. **`render_pose` allocates twice per frame and converts byte-by-byte.**
   `DrawTarget::new` allocates a fresh `width*height` u32 buffer every frame, then the
   ARGB→RGBA conversion does four `out.push(...)` per pixel (~1.2 M pushes per frame).
   - The `Vec::with_capacity` makes pushes amortized-cheap, but a chunked write is both
     faster and clearer:
     ```rust
     let mut out = vec![0u8; (width * height * 4) as usize];
     for (px, dst) in data.iter().zip(out.chunks_exact_mut(4)) {
         dst[0] = (px >> 16) as u8; dst[1] = (px >> 8) as u8;
         dst[2] = *px as u8;        dst[3] = (px >> 24) as u8;
     }
     ```
   - Keeping a reusable `DrawTarget` in `PoseTask` (clear per frame with
     `dt.clear(SolidSource { .. 0 })`) removes the second allocation. `PoseTask` is
     already `&mut`-accessible through `Model::process_rgba`.
   - This dovetails with `cv_buffer_pool_leak.md` — if the CV output pool is kept, the
     `out` buffer should come from it.

2. **`preprocess_rgba` stretches 640×480 → 640×640 without letterboxing.**
   YOLOv8 is trained on letterboxed (aspect-preserved, padded) inputs. Non-uniform
   stretch degrades keypoint confidence, especially at frame edges. Because the same
   `x_ratio/y_ratio` are used to scale keypoints back in `decode_yolo_pose`, geometry is
   *consistent* (angles are computed in original-image space) — so this is an accuracy
   improvement, not a correctness bug. If detection feels flaky at the calibration step
   ("Only N/5 valid samples"), letterboxing is the first lever. Implementation: scale by
   `min(w_ratio, h_ratio)`, pad with gray (114/255 is YOLO convention), and adjust the
   inverse transform in `decode_yolo_pose` to subtract padding before scaling.

3. **`decode_yolo_pose` scans all 8400 anchors with a manual loop** (~line 42). Fine,
   but idiomatic + equally fast:
   ```rust
   let best_row = preds.outer_iter().max_by(|a, b| a[4].total_cmp(&b[4]));
   ```
   Also `scale_x as f32` at ~line 62 casts an `f32` to `f32` — leftover noise.
   And since only `KEEP_KEYPOINTS` (nose + shoulders) are decoded, the full-skeleton
   `SKELETON` drawing in `draw_skeleton` can never draw more than the 0–5/0–6 edges —
   either decode all 17 keypoints for a richer overlay, or shrink `SKELETON` to the
   edges that can actually appear; right now the code implies a full skeleton renders
   but it can't.

## Clues
- `process_rgba` already measures per-stage timings (`TimeMetrics`, shown in the debug
  window) — use the debug window before/after to quantify the render and preprocess
  changes on-device.
- Tests in `pose.rs` cover `preprocess_rgba` output shape/normalization and decode
  scaling; the letterbox change must update
  `preprocess_rgba_produces_normalized_nchw_tensor` and
  `decode_yolo_pose_extracts_and_scales_best_detection` accordingly.
