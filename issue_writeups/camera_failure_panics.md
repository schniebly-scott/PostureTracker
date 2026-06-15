# Camera failure paths panic or die silently instead of surfacing to the UI

**Severity: medium-high — unplugging a webcam or a permissions hiccup can crash the app
or freeze the feed with no user-visible explanation.**

## Where
1. `src/app.rs` `Message::TestPosturePressed` (~lines 538–545):
   ```rust
   self.pipelines.camera_manager.start().expect("Unable to start camera");
   self.pipelines.cv_manager.start().expect("Unable to start model");
   ```
   `.expect` panics the whole app if the configured device is missing/busy (common:
   camera unplugged since last run, another app holds it, permission denied on macOS).
2. `src/camera/cam_worker.rs` ~line 65: `let data = frame.data().unwrap();` — a single
   bad frame from ccap panics the capture *thread*. The thread dies, `running` stays
   true, the UI keeps showing the last frame, and `is_running()` lies forever.
3. `src/app.rs` `begin_background_tracking` (~lines 350–352): `.start().ok()` — the
   opposite extreme: errors are swallowed entirely; background mode "starts" with no
   camera and the user just never gets alerts.
4. Inconsistent contract: `CVManager::start` returns `Ok(())` even when no model is
   loaded (worker exits immediately with an eprintln; see `cv_worker_model_mutex.md`).
5. macOS-specific: there is no camera-permission denied handling anywhere; AVFoundation
   will fail the open, which currently lands in path (1)'s panic.

## Suggested direction
- Add an error surface to the app state, e.g. `pipeline_error: Option<String>` rendered
  as a dismissible banner in the status panel (`status_panel.rs` already has a badge
  system — a "bad"-kind badge with the error text fits the design).
- `TestPosturePressed` / `begin_background_tracking` / `CameraSelected` (restart path,
  ~line 760) all route `Err` into that state instead of `expect`/`ok()`.
- In `cam_worker`, replace `unwrap()` with a `match` that logs and `continue`s (matching
  how `grab_frame` errors are already handled three lines down), and on *persistent*
  failure (N consecutive errors) flip `running` to false and notify — otherwise a
  detached camera spins the 100 ms retry loop forever.
- Consider a `Message::PipelineFailed(String)` so worker threads can report through the
  existing broadcast/subscription machinery rather than only stderr. The `ServiceCore`
  could carry an error slot, or simplest: a dedicated `tokio::sync::watch<Option<String>>`
  in `Pipelines`.

## Clues
- Camera holders to test with: open the device in another app (`cheese`/OBS), unplug
  USB cam mid-session, select a device then unplug before Start.
- `list_cameras()` already refreshes on settings open + a Refresh button; after an
  error banner lands, wire "camera not found" to suggest opening Settings.
- Don't regress the deliberate stale-frame drop in `Message::CamFrame`
  (`src/app.rs` ~line 366) — error recovery that restarts the camera must keep
  `is_running()` semantics intact for that check.
