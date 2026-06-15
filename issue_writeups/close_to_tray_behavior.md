# Closing the main window quits the app — even mid-tracking, even with a tray icon

**Severity: medium (UX decision to review) — likely data/intent loss for a
background-tracker app.**

## Where
- `src/app.rs` `Message::WindowCloseRequested` (~line 491): `window_id ==
  main_window_id` → `Message::QuitRequested` → `iced::exit()`.
- Tray exists precisely to restore a hidden window: `src/app/tray.rs`
  (`Show PostureTracker` / `Quit` menu).
- The deliberate hide path is `Message::HideMainWindowPressed` → `window::minimize`
  (a Settings-page button, `settings_panel.rs::window_actions`).

## Problem
For an app whose main mode is "track in the background and alert me", the most common
muscle-memory action — clicking the titlebar ✕ — kills tracking entirely with no
confirmation. Meanwhile the *intended* flow (minimize + tray restore) is buried on the
settings page. Every comparable tray app (Slack, Discord, syncthing-gtk…) treats ✕ as
hide-to-tray when a tray icon is present.

Note `stop_tracking()` bookkeeping: `Message::QuitRequested` calls `iced::exit()`
directly **without** `self.metrics.stop_tracking()`. The append-only log gets no `Stop`
event, so the open `Start` interval is dropped on next launch (`load_today` ignores
unmatched `Start`s — see `dedupe_metrics_log_parsing.md`). Quitting while tracking
therefore silently loses the session's tracked time. This is a concrete bug independent
of the UX decision.

## Suggested direction
1. When `tray_state.is_some()` and a session is active (`run_mode == Background` or
   metrics session active): ✕ minimizes/hides instead of quitting (optionally with a
   one-time toast/notice "still tracking in the tray").
2. When no tray is available (the Linux no-appindicator fallback path), keep quit
   behavior but flush metrics first.
3. Either way, fix the quit path: `QuitRequested` should call
   `self.metrics.stop_tracking()` (and persist config if dirty) before `iced::exit()`.
   `MetricsStore::stop_tracking` already appends the `Stop` log event — it just needs to
   be invoked.

## Clues
- `has_system_tray()` already exists on `App` for exactly this kind of branching.
- If hide-on-close lands, audit `window::Settings::exit_on_close_request` — it's already
  `false` for all three windows, so close requests fully route through
  `WindowCloseRequested`; the change is contained to that match arm.
- Test manually on Wayland: tray restore relies on appindicator; if the tray failed to
  initialize, ✕-to-hide would strand the app with no window — branch 2 covers this.
