# CV worker buffer pool leaks memory unboundedly

**Severity: high — unbounded memory growth while inference runs.**

## Where
- `src/cv/cv_worker.rs` (the `pool` local in `CVWorker::spawn`, ~line 19 and ~lines 54–58)
- `src/cv/pose.rs` (`render_pose`, ~line 72 — allocates a fresh `Vec` every frame)
- `src/camera/cam_service.rs` (`RgbaBuffer::drop` — pushes the buffer back into the pool)

## Problem
The pooled-buffer pattern works in the camera worker because it is symmetric:
`cam_worker.rs` **pops** a buffer from the pool before capture and `RgbaBuffer::drop`
pushes it back.

The CV worker copies the pattern but only half of it. `CVWorker::spawn` creates a pool
and wraps every rendered overlay in `RgbaBuffer { data: output, pool: pool.clone() }` —
so each dropped inference frame **pushes** its ~1.2 MB buffer (640×480×4) into the pool
Vec. But nothing in the CV path ever **pops**: `PoseTask::render_pose` allocates a brand
new `Vec::with_capacity(w*h*4)` for every frame. The pool is a write-only Vec that grows
by one full frame buffer per inference for as long as the pipeline runs.

At ~10–15 inferences/sec that's roughly 0.7–1 GB of retained memory per minute of
continuous tracking. The Vec is only freed when the worker's pool Arc is dropped after
`stop()`.

## Fix options
1. **Thread the pool into rendering** (matches the camera worker's intent): pass the pool
   into `Model::process_rgba` → `PoseTask::render`, pop a recycled buffer there
   (`pool.pop().unwrap_or_else(|| vec![0; len])`), and clear/overwrite it. Note raqote's
   `DrawTarget` owns its own `Vec<u32>` internally, so true reuse means converting pixels
   into the pooled buffer in `render_pose` instead of building a new `out` Vec.
2. **Simpler: drop pooling for CV frames entirely.** Give `RgbaBuffer` an
   `Option<Arc<Mutex<...>>>` pool (or add a non-pooled constructor) and let CV buffers
   just deallocate. The CV pipeline runs at ~10 fps, so allocation pressure is modest;
   correctness beats the micro-optimization here.

Option 2 is the smaller, safer diff. Option 1 only pays off if combined with the
`DrawTarget` reuse described in `pose_render_efficiency.md`.

## Clues for verification
- Run a continuous session and watch RSS (`ps -o rss= -p $(pgrep posturetracker)`); it
  climbs steadily today and should plateau after the fix.
- A unit test can assert pool length stays bounded: create the pool, simulate N
  `RgbaBuffer` create/drop cycles with pops, assert `pool.lock().unwrap().len() <= K`.
