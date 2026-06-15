# `App::update`'s CvInference arm is 115 lines with triplicated logic

**Severity: medium — readability/maintainability; the alert rules live in three slightly
different copies that can drift.**

## Where
`src/app.rs`, `Message::CvInference` arm (~lines 374–490), plus:
- `Message::PostureThresholdChanged` (~line 564) — re-derives `bad_posture`
- `begin_background_tracking` (~line 334) and `Message::TestPosturePressed` (~line 526)
  — duplicate the lazy model-load block verbatim

## Duplications to extract

1. **Bad-posture predicate (3 copies).** `(current - baseline).abs() >= threshold` is
   written in the CvInference arm, in PostureThresholdChanged, and a third time inside
   the background-sample `filter` closure (~line 460). Extract:
   ```rust
   fn is_bad_posture(baseline: Option<f32>, angle: Option<f32>, threshold: f32) -> bool
   ```
   Free function → trivially unit-testable, and the sample-window filter can reuse it.

2. **Alert-window opening (2 copies).** The `can_alert` check + `window::open` +
   `maximize` block appears at ~lines 422–436 (continuous mode) and ~lines 471–484
   (interval mode), and the paired "auto-dismiss unless force_dismiss" branch is also
   duplicated. Extract something like:
   ```rust
   /// Opens the alert window if cooldown allows; returns the Task to run.
   fn evaluate_alert(&mut self, bad: bool) -> Task<Message>
   ```
   so both sampling strategies funnel through one policy point.

3. **Lazy model load (2 copies).** The `if Unloaded { load_model() ... }` block in
   `begin_background_tracking` and `TestPosturePressed` is identical. Extract
   `fn ensure_model_loaded(&mut self) -> bool`.

4. **Calibration sample handling** (~lines 392–416) is a self-contained state machine
   step — move it into a method on `CalibrationState` or a private
   `fn ingest_calibration_sample(&mut self, angle: Option<f32>)`. The median/min-samples
   logic would become unit-testable (it currently isn't, because it's welded into the
   update loop).

## Suggested shape
Keep `update` as a thin dispatcher; the CvInference arm should read roughly:
```rust
self.apply_inference(frame, time_metrics, angle);   // state + metrics ingest
self.ingest_calibration_sample(angle);
self.evaluate_background(angle)                      // returns Task
```

## Clues
- Be careful with the interval-mode branch: it *stops the pipeline* before deciding to
  alert (`background_samples = None; camera.stop(); cv.stop();` ~line 466) while the
  continuous branch leaves it running (comment at ~line 429 explains why). That
  asymmetry is intentional — preserve it, and keep the explanatory comments.
- `self.update(Message::DismissAlert)` recursion (~line 438) is a self-dispatch; fine to
  keep, but the extracted helper can call the dismiss logic directly instead.
- Existing tests in `src/app.rs` only cover `MetricsCategory`/interval mapping; new
  helpers should each get direct tests (especially `is_bad_posture` and the majority
  vote on `background_samples`).
