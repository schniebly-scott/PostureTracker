# CV worker holds the model mutex for the thread's entire lifetime

**Severity: medium — latent deadlock structure; currently masked by careful call order in
`app.rs`.**

## Where
- `src/cv/cv_worker.rs` ~lines 22–31: `let mut model_lock = self.model.lock().unwrap();`
  acquired at thread start and held until the `while running` loop exits.
- `src/cv/cv_service.rs`: `CVManager::load_model` locks the same `Arc<Mutex<Option<Model>>>`.

## Problem
The worker thread takes the `Mutex<Option<Model>>` guard once and keeps it for its whole
life. Consequences:

1. **`load_model()` blocks while a worker runs.** Today `App::begin_background_tracking`
   and `Message::TestPosturePressed` (`src/app.rs`) only call `load_model` when
   `InferenceState::Unloaded`, so the order happens to be safe — but any future code path
   that reloads the model (e.g. a "switch model" setting) will block the UI thread until
   the worker is stopped *and* notices the atomic flag (up to one frame-poll iteration
   later). Nothing in the type signature warns about this.
2. **Two overlapping `start()` calls serialize silently.** If `start()` is ever called
   while a previous worker is still draining (stop is asynchronous — it just flips the
   `AtomicBool`), the new thread parks on the mutex. With unlucky interleaving the old
   thread exits, the new one proceeds with the *old* shared state — hard to debug.
3. It also means `is_running()==false` does not imply the model lock is free.

## Fix
Move the model out of the shared mutex for the run, or scope the lock per frame:

- **Option A (preferred):** `CVManager::start` takes the model *out* of the mutex
  (`model.lock().unwrap().take()`) and hands it to the worker by value; the worker puts
  it back on exit (or the manager keeps `Model` behind the mutex only between sessions).
  This makes "model in use" explicit and keeps `load_model` non-blocking with a clear
  error ("model is in use") instead of a stall.
- **Option B:** lock inside the loop per frame. Cheap (uncontended mutex) and minimally
  invasive, but keeps the double-start hazard.

Also consider making `start()` reject when `is_running()` is already true (guard in
`ManagedService` default impl, `src/utils.rs`) — that closes the double-spawn hole for
both camera and CV managers in one place.

## Clues
- The worker exits early with only `eprintln!("Model not loaded!")` when the Option is
  None — `start()` returns `Ok(())` even though no work will happen. Whatever fix lands
  should surface this as an `Err` from `CVManager::start` instead (the camera manager
  already returns `Err` for a missing device, so this aligns them).
- Test idea: call `load_model`, `start`, then `load_model` again from another thread with
  a timeout — today it hangs until `stop()`.
