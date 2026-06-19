# Metric values overflow the bottom of the cards at certain window heights (Windows)

**Severity: medium (most visible on Windows) — at some window heights the metric values
spill past the bottom edge of their cards / get clipped by the panel, so numbers are
half-cut or hidden. Cosmetic, but it makes the dashboard look broken.**

## Where
- `src/app/components/metrics_panel.rs`:
  - `view()` ~lines 39–72: the panel is `column![header, body, footer]` inside a
    container with `.height(Length::Fill).clip(true)`. The whole panel **clips** its
    overflow, so when the stacked content is taller than the available height the bottom
    cards get sliced rather than pushed into a scroll area.
  - `view_daily` / `view_session` / `view_all_time` ~lines 139–290: each is a
    `column![primary, row_a, (quality_card)]` of fixed-content cards with fixed paddings
    (`[12,14]`, `[10,12]`) and fixed text sizes (`primary_card` value `size(23)`,
    `secondary_card` value `size(17)`). The column has no min-height guarantee and the
    cards don't shrink — total height is essentially constant regardless of window size.
  - `secondary_card` (~line 396) sets `.clip(true)` on the card itself, so an oversized
    value is cut at the card edge too.
- The column the panel lives in: `src/app.rs` `view()` ~lines 805–816 — control panel +
  metrics panel share `column![...].width(FillPortion(2)).height(FillPortion(7))`.
  Metrics gets whatever vertical space is left after the control panel; there's no
  `Scrollable`, so if that remainder is shorter than the cards, content is clipped.

## Why it's height-dependent, and worse on Windows
The cards are sized by their content (fixed font sizes + fixed padding), but the panel
they sit in is `Fill` with `clip(true)`. When the main window is short — e.g. dragged
small, or at the `MAIN_WINDOW_MIN_SIZE` floor, or on a 768p panel — the camera row
(`FillPortion(7)`) and status panel (`FillPortion(5)`) eat the height and the metrics
column's share drops below the cards' natural height. The fixed-height cards don't
compress, so the last card's value lands outside the visible/clipped region.

Windows makes it appear more often for two reasons:
1. **Display scaling** (125–150 %) shrinks the effective logical workspace, so the
   "short window" condition is the common case, not the edge case.
2. **Font metrics differ.** The bundled vs. system font fallback (see
   [icon_glyphs_cross_platform]) and Windows' text rendering can give the value text a
   slightly larger line box than on Linux/macOS, so values that *just* fit elsewhere
   tip over the card's bottom padding on Windows.

## Suggested direction
1. Make the metrics body scrollable instead of clipped: wrap `view_body`'s content (or
   the per-category column) in an `iced` `Scrollable` so a too-short window degrades to
   a scroll rather than a hard cut. This is the robust fix and pairs with the
   resizable-window work in [responsive_main_window].
2. Or give the metrics panel a sensible `min_height` and let the layout (window
   `min_size`) guarantee enough room, so the clip never triggers in practice.
3. Don't rely on `wrapping(Wrapping::None)` + `clip(true)` to "hide" overflow — that's
   what's cutting the values. Let text measure naturally and size the card to it.
4. Audit the fixed paddings/sizes for vertical breathing room: the value text
   (`size(23)`/`size(17)`) sets the card's intrinsic height; confirm the card's vertical
   padding accommodates the tallest glyph line box on Windows' renderer, not just Linux.

## Acceptance check
On Windows at 100 % and 150 % scaling, resize the window from the min size up to full
height and confirm no metric value is ever clipped or hanging past a card's bottom edge
in any of the Daily / Session / All-Time / Quick views. Repeat on Linux/macOS for no
regression.
