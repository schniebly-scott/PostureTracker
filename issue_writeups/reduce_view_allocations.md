# `view()` hot path allocates avoidably on every redraw

**Severity: low-medium — `view` re-runs on every message; during a session that's ~30×/s
(every camera frame). Individually tiny allocations multiply.**

## Where & what

1. **`settings_panel.rs` `camera_pick_list` (~line 82):**
   `pick_list(app.available_cameras.clone(), ...)` clones the whole
   `Vec<CameraOption>` (each containing two `String`s) per redraw. iced's `pick_list`
   accepts `impl Borrow<[T]>` — pass `&app.available_cameras` (or
   `app.available_cameras.as_slice()`) and keep `selected` as the only clone.
   This view is cheap to hit because the settings page redraws on every slider/input
   message too.

2. **`ui.rs` `seg_button` (~line 229):** `text(label.to_string())` — `text()` accepts
   `&'a str` directly; the `to_string` is pure waste. Same pattern in
   `control_panel.rs::glyph_label` (~line 25): `text(glyph.to_string())` and
   `text(label.to_string())` where borrowing works (the function returns
   `Element<'static>` — change to a lifetime-parameterized return or accept
   `&'static str`, which all call sites already pass).

3. **Per-frame `format!` strings** in `camera_panel::angle_chip`, `status_panel`
   (`state_line`, threshold value), `metrics_panel` (every card) are unavoidable for
   dynamic values — fine. But static strings passed through `format!`/`to_string` where
   a literal suffices should be literals (grep `to_string()` in `src/app/components/`;
   several are on fixed strings like `"--".to_string()` where `text("--")` works
   when the match arms can produce `Cow`/`String` only for the dynamic case).

4. **`Message` enum cost:** `Message::CvInference((image::Handle, TimeMetrics, Option<f32>))`
   and `CamFrame(image::Handle)` are fine — `Handle` is an `Arc` internally, cheap to
   clone. No action needed there; noted so nobody "optimizes" it.

## Why it matters beyond perf
The codebase's stated style goal (claude.md) is "avoid clone whenever possible". The
view layer is where most remaining clones live; fixing the two structural ones (#1, #2)
also makes the idiom obvious for future panels.

## Clues
- After changing `glyph_label` to borrow, button labels currently typed as
  `Element<'static, Message>` may need a lifetime: `fn btn_label<'a>(glyph: &'a str,
  label: &'a str) -> Element<'a, Message>` — call sites all pass literals so `'static`
  inference still holds.
- Don't chase `Handle::clone` in `camera_panel::view` (~lines 151/156) — that's an Arc
  bump, required by `image()` taking ownership.
- Quick audit command: `grep -n "\.clone()\|to_string()" src/app/components/*.rs`.
