# Unicode glyph "icons" depend on system font fallback — will render inconsistently per OS

**Severity: medium — every button/chip icon is a unicode character whose presence, width
and vertical metrics differ between Linux/Windows/macOS font stacks.**

## Where (grep for `\u{` in `src/app/components/`)
- `control_panel.rs`: `\u{25A0}` ■, `\u{25B6}` ▶, `\u{25CE}` ◎, `\u{2026}` …, `\u{2699}` ⚙
- `camera_panel.rs`: `\u{25A3}` ▣, `\u{25CE}` ◎, `\u{00B0}` °, `\u{2014}` —
- `metrics_panel.rs`: `\u{25F7}` ◷, `\u{25B3}` △, `\u{2715}` ✕, `\u{25CF}`/`\u{25CB}` ●○,
  `\u{25D0}` ◐, `\u{2261}` ≡, `\u{25A4}` ▤, `\u{2197}` ↗, `\u{21BB}` ↻, `\u{2039}/\u{203A}` ‹›
- The workaround that proves the problem: `control_panel.rs` `glyph_label` (~line 21)
  already pins `LineHeight::Absolute(20.0)` because ■ and ▶ size buttons differently.

## Problem
iced's default font is whatever the bundled/system sans resolves to per platform.
Geometric shapes (U+25xx) and dingbats are not guaranteed in those fonts; when missing,
the renderer falls back to another font (different advance widths, baselines) or shows
tofu (□). Concretely:
- On Windows (Segoe UI), several of these (◷ ◐ ▤) commonly come from fallback fonts with
  visibly different weight/size than neighbors.
- Vertical alignment hacks tuned on Linux (the absolute line-height) may be wrong on
  another fallback font, skewing every button label.
- Width assumptions skew layouts: chips/buttons get wider or narrower per OS, which
  matters in the fixed-width panels (see `responsive_main_window.md`).

## Fix options (pick one, apply everywhere)
1. **Bundle an icon font** (e.g. Lucide/Phosphor/Material Symbols subset) via
   `iced::Settings::fonts` / `font::load`, and add `ui::icon(glyph) -> Text` that always
   sets that font. Guarantees identical rendering on all three OSes. ~30 KB asset.
2. **Bundle a known text font for the whole app** (Inter + JetBrains Mono are already
   the design intent — `ui.rs` says "JetBrains-Mono stand-in") and verify every glyph
   used exists in it. This also fixes the broader "design tuned on one font" problem and
   makes `ui::mono()` actually JetBrains Mono.
3. Replace glyphs with tiny `canvas`/SVG icons — most robust, most work.

Option 1+2 combined is the sweet spot: bundle fonts, route all icon usage through one
`ui.rs` helper so future agents can't introduce raw glyph strings.

## Clues
- Centralizing icons in `ui.rs` lets the `glyph_label` line-height hack be deleted —
  with a single known font the metrics are stable.
- `Cargo.toml` already enables the `image`/`canvas` features; loading fonts needs
  `iced::application(...).font(BYTES)` or daemon equivalent in `src/app.rs::run`.
- Check the tray tooltip/menu too (`src/app/tray.rs`) — those render with native OS
  menus, so they are *not* affected; only in-window text is.
