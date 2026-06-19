# Width-expansion priority: pad first, then grow camera/graph, keep controls fixed

**Severity: medium (UI polish, cross-platform) — the window is now resizable
([responsive_main_window]), but at wide widths everything stretches proportionally,
including the controls/metrics column that should stay a fixed, readable width. The
expansion needs an explicit priority order so wide windows look composed, not stretched.**

## Desired behavior (in priority order, narrow → wide)
1. **First**, take up slack as *padding around the cards* — let the outer gutter grow a
   bit before any card grows.
2. **Then** the **camera section expands** and the **status graph expands slightly** to
   use real estate.
3. The **controls / metrics column stays a fixed width** (it's text + inputs; widening
   it just adds dead space and hurts readability).
4. The **status panel only expands so far**: past some width, only the *graph* inside it
   keeps growing, and the panel itself can sit **centered with large left/right padding**
   rather than stretching edge-to-edge.

## Where (current layout)
- `src/app.rs` `view()` ~lines 804–822 — dashboard body:
  ```
  column![
      row![
          camera_panel::view        // width FillPortion(3)
          column![control, metrics] // width FillPortion(2)   <-- should be FIXED
      ].height(FillPortion(7)),
      status_panel::view,
  ]
  ```
  wrapped in `container(body).padding(14)`. Today both columns scale with
  `FillPortion`, so the controls/metrics column widens with the window (violates #3), and
  the outer padding is a constant `14` (violates #1).
- `src/app/components/camera_panel.rs` ~line 196: camera panel `width(FillPortion(3))`.
- `src/app/components/status_panel.rs`:
  - `view()` ~lines 234–260: `row![left, graph_card]`, panel `padding(16)`, `width(Fill)`,
    `height(FillPortion(5))`. The panel stretches full width (violates #4).
  - left column ~lines 244–249: already `width(FillPortion(2)).max_width(366.0)` — good,
    this is the pattern to copy for the dashboard controls column.
  - `graph_card` ~lines 226–229: `width(FillPortion(3))` — this is the part that *should*
    keep expanding inside the status panel.

## Suggested direction
1. **Fix the controls/metrics column width (#3).** Replace its `FillPortion(2)` in
   `app.rs` with a fixed/bounded width, e.g. `Length::Fixed(360.0)` (or
   `FillPortion(2).max_width(...)` like the status-panel left column already does). Let
   the camera panel be `Fill` (or keep `FillPortion(3)` against a now-fixed sibling) so
   the camera absorbs horizontal slack (#2).
2. **Pad-before-grow (#1) and cap-then-center (#4).** Wrap the dashboard body (and/or the
   status panel's inner `row`) in a `container(...).max_width(W).center_x(Fill)`. Below
   `W` the content fills and the camera/graph grow; above `W` the content stops growing
   and the leftover space becomes symmetric left/right padding — i.e. it stays centered
   with large gutters. Choose `W` so the camera + fixed controls column look balanced
   (roughly current default width, ~1100–1300 logical px) before centering kicks in.
3. **Let the graph keep growing inside a capped status panel (#4).** With the status
   panel's outer width capped/centered, keep `graph_card` at `FillPortion(3)` and the
   left controls at `max_width(366)` so the *graph* is what consumes the remaining width
   inside the panel — exactly the "only the graph part expands inside the panel" goal.
4. Optionally let the outer `container(body).padding(14)` padding scale up at very wide
   widths (or just rely on the centered max-width gutters from #2, which achieves the
   same "more padding around the cards first" effect without per-width branching).

## Acceptance check
Drag the window from `MAIN_WINDOW_MIN_SIZE` to a very wide width and watch the order:
gutters grow first, then the camera and the status graph grow, while the controls/metrics
column and the status-panel controls stay a constant readable width. At very wide widths
the status panel (and ideally the whole dashboard) is centered with large side padding
rather than stretched. Verify at 100 % and 150 % Windows scaling.
