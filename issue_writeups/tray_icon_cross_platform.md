# Tray icon is a procedurally drawn placeholder; won't look native on macOS/Windows

**Severity: low-medium — first thing users see of the "background app" identity on two
of the three target platforms.**

## Where
- `src/app/tray.rs` `app_icon()` (~line 169): generates a 32×32 blue square with a cross
  accent, pixel by pixel, at runtime.
- `TrayIconBuilder` setup in `build_tray_icon` (~line 64).

## Problems per platform
- **macOS:** menu-bar icons are expected to be *template images* (monochrome, alpha
  only) so the OS can tint them for light/dark menu bars and accessibility modes. A
  solid blue/navy square will clash with both appearances. `tray-icon` supports
  `with_icon_as_template(true)` — required for a native look.
- **Windows:** 32×32 is wrong at common DPI scales (the shell wants 16×16 @ 1x scaled
  up; a multi-size .ico-derived asset renders crisper). The blue square reads as a
  generic placeholder in the system tray.
- **Linux:** appindicator themes vary wildly; the current icon at least shows up
  (verified by the existing fallback logic), but a real asset would also fix the blurry
  upscale on HiDPI panels.
- The icon also can't reflect state — a posture tracker's tray icon is the natural place
  to show green/red status while minimized (`TrayIcon::set_icon` allows swapping at
  runtime; `App` already knows `bad_posture`).

## Suggested direction
1. Design one simple mark (spine/chair silhouette) exported at 16/24/32/48 px, embedded
   with `include_bytes!` and decoded with the already-present `image` crate
   (`Icon::from_rgba` stays the entry point).
2. On macOS pass a monochrome variant + `with_icon_as_template(true)`
   (`#[cfg(target_os = "macos")]`).
3. Optional follow-up: swap icon on posture state change while in background mode —
   plumb a `tray_state.set_status(bad: bool)` call from the CvInference arm in
   `src/app.rs`. Keep it cheap: only call when the state *changes*.

## Clues
- `TrayState` currently stores the menu items only to keep them alive (the `_`-prefixed
  fields) — adding a `tray_icon: TrayIcon` accessor for `set_icon` means un-prefixing
  `_tray_icon`.
- The panic-hook dance in `build_tray_icon` exists because tray creation can panic on
  Linux without a tray host; don't disturb it — test any change on a trayless session
  (e.g. bare sway) to confirm graceful degradation still works.
- Windows packaging (CI workflow `.github/workflows/rust.yml`) doesn't embed an .exe
  icon either (`winres`/`embed-resource` crate would) — same asset can serve both; note
  it in the PR but it's a separate change.
