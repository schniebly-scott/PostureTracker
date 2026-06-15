# Alert window: borderless+maximize+72 px text won't behave the same on every OS

**Severity: medium — the alert is the product's core moment; it currently depends on
platform-specific window-manager behavior.**

## Where
- `src/app.rs` `alert_window_settings()` (~line 903): `decorations: false`,
  `level: AlwaysOnTop`, size 1000×600, `Position::Centered`.
- `src/app.rs` CvInference arm (~lines 433 & 478): opens the window then immediately
  `window::maximize(id, true)`.
- `src/app/components/alert_overlay.rs`: fixed `text(...).size(72)` and `.size(24)`.

## Problems
1. **Maximize semantics differ.** On macOS, "maximize" historically maps to zoom and
   interacts oddly with undecorated windows; on Linux it depends on the WM honoring the
   request for an undecorated, always-on-top surface (tiling WMs frequently ignore it).
   The result ranges from full-screen red (intended) to a floating 1000×600 box.
2. **Two sources of truth for size.** The settings say 1000×600 but the code immediately
   maximizes — whichever wins, the other is dead config. If maximize fails, 1000×600 is
   the fallback the design never accounted for.
3. **Fixed type sizes don't scale.** 72 px headline is tuned for a maximized 1080p+
   window; in the 1000×600 fallback (or a small laptop) it dominates, while on a 4K
   maximized window it looks small relative to the field of red.
4. **No keyboard dismissal.** `mouse_area` only — Escape/Enter should dismiss too
   (accessibility + muscle memory), especially since the window covers the screen.
5. On multi-monitor setups `Position::Centered` + maximize picks a monitor by
   platform-specific rules; the alert may appear on a screen the user isn't looking at.

## Suggested direction
- Prefer `window::Settings { fullscreen: ... }`-style explicit full-screen if/where iced
  0.14 supports `window::change_mode(id, Mode::Fullscreen)`; otherwise keep maximize but
  make the fallback intentional: design the 1000×600 layout to look correct (it's the
  guaranteed baseline), and treat maximize as progressive enhancement.
- Scale the headline with the window: wrap the column in `responsive()` (iced widget) and
  derive text size from width buckets, or simply use sizes that read well at 1000×600.
- Add a `keyboard::on_key_press` subscription (active only while
  `alert_window_id.is_some()`) mapping Escape → `Message::DismissAlert`.
- Test matrix to record in the PR: GNOME/Wayland, KDE/X11, Windows 11, macOS — what the
  WM actually does with undecorated+maximize+always-on-top.

## Clues
- The dismissal/cooldown logic is in `Message::WindowCloseRequested` and
  `Message::DismissAlert` (`src/app.rs` ~lines 497 & 643) — both set `last_alert_time`;
  any new dismissal path must do the same or the cooldown silently won't start.
- `force_dismiss` (config `background.force_dismiss`) decides whether the alert
  auto-closes when posture corrects; don't let a keyboard-dismiss change land that
  bypasses the `MIN_ALERT_COOLDOWN_SECS` floor enforced in `CooldownInputChanged`.
