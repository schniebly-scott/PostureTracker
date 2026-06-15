# Minor idiomatic cleanups (grouped — each is a few-line fix)

Small, independent items; none change behavior. Grouped to avoid a dozen one-line issue
files. Good "first task" for warming up an agent on the codebase.

## 1. `Box<PoseTask>` is pointless
`src/cv/cv_inference.rs` ~line 14: `task: Box<PoseTask>` — `PoseTask` is three small
fields (`usize`, `usize`, `f32`); boxing adds indirection for nothing. Store it inline.
While there: `PoseTask`'s fields just mirror `INF_WIDTH`/`INF_HEIGHT`/
`CONFIDENCE_THRESHOLD` constants (`src/constants.rs`); either keep the fields (they make
the struct configurable/testable — fine) or use the constants directly, but not the
current halfway state where `PoseTask::new()` has no parameters at all.

## 2. Stringly-typed badge kind
`src/app/components/status_panel.rs` `status_state` returns
`(&'static str, &'static str, Color)` where the second field is `"neutral"|"ok"|"bad"`
matched by string in `state_badge` (~line 27). Make it a tiny enum
(`enum BadgeKind { Neutral, Ok, Bad }`) — the compiler then catches typos and the
`match kind { "neutral" => …, _ => … }` wildcard stops silently absorbing new states.
Also `state_badge` and `state_line` re-derive overlapping conditions from `app` — a
single `fn live_status(app) -> (BadgeKind, &'static str, String)` would keep badge and
sentence in sync.

## 3. `Message::CvInference` tuple payload
`src/app.rs` ~line 181: `CvInference((image::Handle, TimeMetrics, Option<f32>))` — a
tuple inside a tuple-variant, destructured positionally at the use site. There's already
a `TODO` acknowledging this in `src/app/subscriptions.rs` (~line 81). Introduce a small
struct (e.g. `InferenceUpdate { handle, time_metrics, posture_angle_deg }`) built in the
subscription, so field names appear at the `update()` site.

## 4. `HISTORY_SECS` defined twice
`src/metrics.rs` ~line 10 and `src/app/components/debug_stats.rs` ~line 12 both define
`const HISTORY_SECS: f64 = 120.0;`. They must agree (the chart's x-axis assumes the
store's retention window) but nothing enforces it. Export it from `metrics.rs`
(`pub const`) and import in `debug_stats.rs`. The status panel's hardcoded label
"LAST 2 MIN" (`status_panel.rs` ~line 201) could be formatted from it too.

## 5. `ServiceCore` field visibility
`src/utils.rs`: `running` and `tx` are `pub`, so workers reach into internals
(`self.core.running.store(...)`, `self.core.tx.send(...)`). Adding
`ServiceCore::publish(&self, value)` and `mark_running(bool)` (or `pub(crate)` fields)
would shrink the contact surface — relevant if the start/stop guard from
`cv_worker_model_mutex.md` lands in the trait.

## 6. Subscription recipes hash only `TypeId`
`src/app/subscriptions.rs` (`hash` impls ~lines 38 & 83): two different recipe types is
fine today, but the hash ignores the receiver entirely — if subscriptions are ever
parameterized (per-camera, per-window), identical hashes would silently dedupe them.
Leave a comment or hash a discriminator now; this is a tripwire, not a bug.

## 7. Dead `InfType` enum
`src/cv.rs` ~lines 27–32: `InfType { Pose, BoundingBox, Segment }` derives
Serialize/Deserialize but has zero usages (`grep -rn InfType src/`). Either it's the
seed of a planned model-selection feature (then wire it into config) or it should be
deleted — dead serde types invite confusion about what's persisted.

## 8. `cstr_to_string`-adjacent: duplicated `v4l2` doc constant
`src/camera.rs` `v4l2::set_yuyv` hardcodes width/height byte offsets into a `[u8; 200]`
union blob — correct, but a `#[repr(C)] struct V4l2PixFormat` for the leading fields
would let the compiler do the offsets. Only worth it if this module grows another ioctl.
