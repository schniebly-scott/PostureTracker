# Fixed 1206×961 non-resizable main window won't fit common laptop screens

**Severity: high for cross-platform — the app is unusable on 1366×768 panels and cramped
under Windows 125–150 % display scaling.**

## Where
- `src/app.rs` ~lines 21–23: `MAIN_WINDOW_SIZE: Size = Size::new(1206.0, 961.0)`,
  `DEBUG_WINDOW_SIZE`, `ALERT_WINDOW_SIZE`; `main_window_settings()` sets
  `resizable: false` and `level: AlwaysOnTop`.
- Fixed dimensions scattered through the layout that the window size was tuned around:
  - `src/app.rs` `view()` ~line 797: right column `width(Length::Fixed(462.0))`
  - `src/app/components/control_panel.rs` ~lines 180 & 195: `height(Length::Fixed(210.0))`
    **twice** (panel + gear overlay must stay in sync by hand)
  - `src/app/components/status_panel.rs` ~line 245: left column `Fixed(366.0)`
  - `src/app/components/metrics_panel.rs`: `progress_bar(...).length(100)` / `.length(120)`,
    reset-popup offset `padding([56, 16])`
  - `src/app/components/settings_panel.rs`: `FIELD_WIDTH = 300.0`, prompt card 280px

## Why this breaks across devices
iced sizes are logical pixels, so OS scaling is handled — but the *screen budget* isn't:
961 logical px of height doesn't fit a 768-px panel at 100 % scale, and at 150 % scale on
a 1080p Windows laptop the effective workspace is only 1280×720 logical. The window also
can't be resized or maximized to compensate. On small screens users will get a window
whose bottom (the entire status panel with threshold slider and interval picker) is off
screen with no recourse.

`AlwaysOnTop` on the *main* window compounds it: an oversized, unresizable,
always-on-top window covers other apps and can't be pushed back. (AlwaysOnTop is right
for the *alert* window; questionable for the dashboard.)

## Suggested direction
1. Make the main window `resizable: true` with `min_size` ≈ 980×640, default size
   clamped to something like 1100×860. The layout already uses `Fill`/`FillPortion(7)/(5)`
   for the big regions, so it mostly reflows — the fixed widths above are what skew.
2. Convert the fixed widths to proportions or bounded fills:
   - right column: `Length::Fixed(462.0)` → `Length::FillPortion(2)` against the camera
     panel's `FillPortion(3)`, or keep fixed but with the window min-size guaranteeing it.
   - status panel left column: `Fixed(366.0)` → `FillPortion` + `max_width`.
   - control panel: replace the duplicated `Fixed(210.0)` with one shared const, or
     better, let content size the panel and reserve space via a fixed-height *button
     slot* (the comment explains the height exists to avoid layout jumps when the start
     row appears — keep that goal).
3. The metrics reset popup's `padding([56, 16])` positions it above the footer by magic
   number; anchor it with `align_y(End)` plus the footer's actual height, or switch to
   an `iced` overlay/`stack` anchored to the footer row instead of the whole panel.
4. Re-test the camera feed at non-4:3 window proportions — `ContentFit::Contain` on the
   stacked `image`s (camera_panel.rs ~lines 148–160) already keeps the overlay and feed
   aligned since both are the same resolution; that part is safe.

## Acceptance check
Run with `WINIT_X11_SCALE_FACTOR=1.5 cargo run` (or on an actual 768p/150 % machine) and
confirm: window fits, all controls reachable, no clipped panels at min size.
