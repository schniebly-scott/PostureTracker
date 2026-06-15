# Config file is rewritten on every keystroke in text inputs

**Severity: low — disk churn + a UX edge case, easy fix.**

## Where
`src/app.rs`:
- `Message::CustomIntervalInputChanged` (~line 606): sets the input and calls
  `save_config()` per keystroke.
- `Message::CooldownInputChanged` (~line 612): parses and saves per keystroke (only when
  valid, but typing "300" saves at "3", "30", and "300").
- `Message::PostureThresholdChanged` deliberately does *not* save per tick — saving
  happens in `PostureThresholdReleased` (slider release). That's the right pattern; the
  text inputs just predate it.

## Problems
1. Each keystroke does a full TOML serialize + `fs::write`
   (`Config::save`, `src/config.rs` ~line 123). Harmless on SSDs, but it's exactly the
   "wasted work" category the project's style goals call out, and on the interval field
   it also rebuilds the iced subscription set (the background timer's `time::every`
   period changes when the parsed value changes — `subscription()` ~line 841).
2. Typing intermediate values can persist unintended state: typing "15" for a custom
   interval transiently saves `interval_secs = 60` (the `or(Some(60))` fallback in
   `sample_interval_secs` ~line 280 applies while input is "1" → 60s) — if the app dies
   at that moment, "1 minute" is what's on disk.

## Fix
Mirror the slider pattern: keep keystrokes in-memory only, save on a "commit" event —
`text_input::on_submit` (Enter) plus saving when the field loses relevance (settings
page closed: `Message::CloseSettingsPressed`; interval choice changed away from Custom).
A tiny `config_dirty: bool` flag on `App` with a save in those exit points covers both
fields, and is also where a quit-time flush belongs (see `close_to_tray_behavior.md`).

## Clues
- `save_config` (~line 290) currently *also* re-derives `config.background.interval_secs`
  from UI state — whoever implements the dirty-flag should keep that derivation in one
  place so deferred saves don't write stale values.
- Watch the integration test `tests/config_integration.rs` — it exercises load/save
  round-trips and is the place to pin the new commit semantics if they become
  observable.
