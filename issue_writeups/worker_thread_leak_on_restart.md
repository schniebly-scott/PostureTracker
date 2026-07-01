# Worker threads leak on stop→start restarts: the old worker re-arms itself

**Severity: high — every camera/resolution change while a session is live leaked one
OS thread holding an open camera handle; thread count climbed visibly in a process
monitor and the leaked session kept publishing frames / contending for the device.**

## Where
- `src/utils.rs` `ServiceCore` — a single `Arc<AtomicBool>` `running` flag was reused
  across sessions and shared by the manager and every worker it ever spawned.
- `src/camera/cam_worker.rs` — the capture loop was `while self.core.is_running()`,
  re-reading that shared flag between `grab_frame(3000)` calls.
- `src/app.rs` `Message::CameraSelected` / `Message::CaptureResolutionSelected` — both
  do `camera_manager.stop()` immediately followed by `camera_manager.start()` in the
  same update.

## The race
1. `stop()` stores `running = false`. The old worker doesn't see it: it's blocked
   inside `camera.grab_frame(3000)` (up to 3 s; ~33 ms with frames flowing).
2. `start()` runs in the same UI update, `claim_running()` CAS-es the *same* flag back
   to `true`, and spawns a second worker.
3. The old worker returns from `grab_frame`, checks `is_running()`, sees `true`, and
   keeps looping — forever. Nothing distinguishes "my session" from "the service".

Each restart therefore leaked one thread plus its ccap `Provider` (open V4L2 device),
buffer pool, and in-flight frames. On Linux the new session's open of the *same*
device then failed with EBUSY while the leaked session kept streaming at the old
settings, so resolution changes could also silently not take effect. Reproduced
empirically: 8 restarts took the process from 4 to 12 threads (`tests/thread_leak_repro.rs`).

The CV worker had the sibling race with a different failure mode: on stop→start the
model hadn't been returned to the shared slot yet, so `spawn_worker` failed with
"model is not loaded" and the session silently didn't start.

## Fix
- `ServiceCore` now holds a **per-session** run flag: `claim_running()` replaces the
  `Arc<AtomicBool>` with a fresh one for each session. Workers snapshot their own
  session's flag at spawn (`session_flag()`) and loop on that, never on
  `is_running()`. A stale worker's flag stays false forever, so it always exits.
- `ServiceCore` also keeps the spawned worker's `JoinHandle` (`adopt_worker` /
  `take_prev_worker`) so a new session can join its predecessor:
  - The **camera** worker joins the previous thread *inside* the new worker thread
    (never on the UI thread, where the 3 s grab timeout would stall input) before
    opening the device, so the old provider has released it. Device open moved into
    the worker thread for the same reason; open failures mark the session not
    running so the UI and a later `start()` see reality.
  - The **CV** manager joins the previous worker synchronously in `spawn_worker`
    before taking the model — bounded to at most one in-flight inference, since
    `stop()` has already woken the frame channel — fixing the "model is not loaded"
    restart failure.

## Verification
- `tests/thread_leak_repro.rs` (ignored; needs `/dev/video0`): threads stayed at 4
  across 8 rapid restarts and dropped to 2 after stop; pre-fix it climbed 4 → 12.
- `src/utils.rs` unit test `restart_does_not_rearm_previous_sessions_flag` pins the
  flag semantics.
