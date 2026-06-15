# Hardcoded color constants duplicate theme tokens; legacy aliases linger

**Severity: medium — violates the project's own "colors only from theme.rs" rule
(claude.md / UI style); byte-identical token copies will silently drift on the next
palette tweak.**

## Where
- `src/app/components/metrics_panel.rs` ~lines 57–78: `CARD_BG`, `PRIMARY_BG`, `POPUP_BG`
  are literal re-encodings of the `ELEV` (0x23282F), `HOVER` (0x2A3038) and `PANEL`
  (0x1B1F25) tokens — the comments even say so ("matches the new design's ELEV token").
- `src/app/components/camera_panel.rs` ~lines 10–23: `FRAME_BG`, `CHIP_BG` are local
  colors not present in the palette at all.
- `src/app/components/settings_panel.rs` ~lines 7–12: another, *different* `POPUP_BG`
  (0x252932).
- `src/app/theme.rs` ~lines 71–77: legacy aliases `OWHITE`, `LIGHT_BLUE`, `MID_BLUE`,
  `DARK_BLUE`, `WARNING_RED` mapped onto the new tokens "for the settings / debug /
  alert screens".
- `src/app/components/metrics_panel.rs` `lighten()` (~line 47) overlaps with
  `ui::mix(color, Color::WHITE, t)` from `src/app/components/ui.rs`.
- metrics_panel also defines its own `bold()` font helper while the style kit exposes
  `ui::semibold()` — the dashboard mixes two type weights for the same role.

## Fix
1. Replace the three metrics_panel constants with the tokens they copy
   (`ELEV`, `HOVER`, `PANEL`) — pure mechanical substitution.
2. Promote `FRAME_BG` and `CHIP_BG` into `theme.rs` as named tokens (e.g. `VIDEO_BG`,
   `SCRIM`) since the camera panel is a first-class design surface; or derive `CHIP_BG`
   from `BG` + alpha via `ui::with_alpha`.
3. Pick one popup surface token (`PANEL` fits) and use it in both metrics_panel's
   `reset_popup` and settings_panel's `camera_prompt`.
4. Delete `lighten()` in favor of `ui::mix(c, Color::WHITE, t)`.
5. Migrating the legacy aliases is the bulk of the work and overlaps with
   `restyle_settings_panel.md`; at minimum stop *adding* new usages. `metrics_panel.rs`
   currently uses `LIGHT_BLUE`/`OWHITE`/`WARNING_RED`/`DARK_BLUE`/`MID_BLUE` heavily —
   mapping: `OWHITE→T1`, `LIGHT_BLUE→GREEN`, `WARNING_RED→RED`, `MID_BLUE→ELEV`,
   `DARK_BLUE→PANEL` (the aliases in theme.rs are exactly these, so it's find/replace
   plus a visual check).

## Acceptance check
`grep -rn "Color {" src/app/components/ | grep -v ui.rs` should return only colors built
from theme tokens (alpha variants etc.), and `grep -rn "OWHITE\|LIGHT_BLUE\|MID_BLUE\|DARK_BLUE\|WARNING_RED" src/` should come back empty, allowing the alias block in
`theme.rs` to be deleted.
