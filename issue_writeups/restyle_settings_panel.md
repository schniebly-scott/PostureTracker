# Settings page and first-run camera prompt don't use the "Refined Slate" style kit

**Severity: medium — the most-visited secondary screen looks like a different app from
the dashboard.**

## Where
- `src/app/components/settings_panel.rs` — the whole file.
- Compare with the style kit: `src/app/components/ui.rs` and tokens in
  `src/app/theme.rs`.

## Problem
The dashboard panels (camera, control, metrics, status) were redesigned around the
shared kit: `ui::panel_style` cards, `ui::micro_label` section headers,
`ui::primary/secondary/danger/ghost_button`, semibold labels + mono values. The settings
page predates that pass:

- Background is `DARK_BLUE` (legacy alias of PANEL) painted edge-to-edge — no card
  surfaces, no hairline borders, so it visually flattens compared to the dashboard.
- Section headers are plain `text("Camera").size(18)` instead of `ui::micro_label`.
- All buttons use a local `flat_button` (~line 16) with legacy colors instead of the kit
  buttons. "Quit App" is a destructive action rendered identically to "Refresh".
- The first-run `camera_prompt` modal defines its own `POPUP_BG` and a `LIGHT_BLUE`
  border instead of the kit's `tile_style`/`panel_style` and `GREEN` accent.
- Fixed widths: `FIELD_WIDTH: f32 = 300.0` and the 280px prompt card (see
  `responsive_main_window.md` for the general fixed-width concern).

## Suggested direction
- Wrap each section (Camera / Posture Alerts / Window) in a `ui::panel_style` container
  with a `ui::micro_label` header — same rhythm as the dashboard columns.
- `flat_button` → `ui::secondary_button`; "Quit App" → `ui::danger_button`;
  the prompt's "Confirm" → `ui::primary_button` (it's the page's one CTA).
- The disabled Confirm state (no `on_press`) should use `ui::disabled_button` so it
  *looks* disabled — today it renders like an active button that ignores clicks.
- `camera_prompt`'s dim scrim (`Color { a: 0.7, ..BLACK }`) is fine; the card itself
  should be `panel_style` + `LINE` border.

## Clues
- The debug window (`debug_stats.rs`) is intentionally unstyled per the project notes
  ("not a pretty or stylized UI") — do **not** drag it into this pass, except its legacy
  alias usage which `consolidate_theme_tokens.md` covers.
- `window_actions` conditionally hides the Debug Window button when the window is open
  (~line 107) — a row that disappears shifts the buttons below it; prefer a disabled
  state over removal so the layout is stable.
- The cooldown warning text is good UX; keep it, but it currently constrains width with
  `Length::Fixed(FIELD_WIDTH)` — let it fill and wrap instead.
