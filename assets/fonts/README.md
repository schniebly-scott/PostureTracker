# Bundled fonts

These fonts are embedded into the binary (`include_bytes!` in
`src/app/components/ui.rs`) and loaded at startup in `app::run`. Bundling them
means every glyph resolves to the same face on Linux, Windows and macOS instead
of going through per-OS system-font fallback — which is what made the old
geometric/dingbat "icons" render at different widths and baselines per platform.

| File | Family (as queried by iced) | Role |
| --- | --- | --- |
| `Inter-Regular.ttf`  | `Inter` (weight 400)         | Default text font |
| `Inter-SemiBold.ttf` | `Inter` (weight 600)         | `ui::semibold()` — labels |
| `Inter-Bold.ttf`     | `Inter` (weight 700)         | `ui::bold()` — values, titles |
| `JetBrainsMono-Regular.ttf` | `JetBrains Mono` (400) | `ui::mono()` — numbers |
| `PostureIcons.ttf`   | `Posture Icons`              | `ui::icon()` — every UI glyph |

`PostureIcons.ttf` is a subset of DejaVu Sans containing only the 17 glyphs the
UI uses (see `ui::glyph`). The Inter / JetBrains Mono files are subset to Latin +
General Punctuation to keep the binary small.

## Licenses

- **Inter** — SIL Open Font License 1.1, © The Inter Project Authors
  (https://github.com/rsms/inter).
- **JetBrains Mono** — SIL Open Font License 1.1, © 2020 The JetBrains Mono
  Project Authors (https://github.com/JetBrains/JetBrainsMono).
- **PostureIcons** (DejaVu Sans subset) — DejaVu Fonts License (Bitstream Vera
  derivative), a permissive license (https://dejavu-fonts.github.io/License.html).

The OFL permits bundling and redistribution; the original license texts ship
with the upstream projects linked above.

## Reproducing these files

Built from the upstream variable fonts with `fonttools` (static-instanced,
then subset):

```sh
# Inter (regular/semibold/bold) and JetBrains Mono (regular) from Google Fonts
#   ofl/inter/Inter[opsz,wght].ttf
#   ofl/jetbrainsmono/JetBrainsMono[wght].ttf
# fontTools.varLib.instancer -> instantiateVariableFont({wght: 400|600|700})
# fontTools.subset           -> unicodes 0x20-0x24F, 0x2000-0x206F (+ a few)
#
# Icon font: subset /usr/share/fonts/.../DejaVuSans.ttf to the 17 glyphs in
# `ui::glyph` and rename the family to "Posture Icons".
```
