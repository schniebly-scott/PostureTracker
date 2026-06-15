# Issue writeups index

Review pass of 2026-06-12 (code: readability/structure/idiomatic Rust/memory;
UI: cross-platform layout & consistency). Each file is self-contained for handoff.

## Memory & performance
| File                                                         | Summary                                                                                               | Severity     |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------- | ------------ |
| ~~[cv_buffer_pool_leak.md](cv_buffer_pool_leak.md)~~         | ~~CV worker's buffer pool is write-only → unbounded RAM growth (~1.2 MB/frame) while inference runs~~ | ~~**High**~~ |
| [eliminate_frame_clones.md](eliminate_frame_clones.md)       | Three full-frame `Vec` clones per frame (UI handle ×2, CV input ×1); zero-copy fixes available        | Med-high     |
| [pose_render_efficiency.md](pose_render_efficiency.md)       | Per-frame DrawTarget alloc + byte-push pixel loop; letterboxing note for model accuracy               | Low-med      |
| [reduce_view_allocations.md](reduce_view_allocations.md)     | `view()` hot-path clones: pick_list Vec clone, `to_string()` on static labels                         | Low-med      |
| [config_save_per_keystroke.md](config_save_per_keystroke.md) | TOML rewrite on every keystroke; transient values can persist                                         | Low          |

## Correctness & robustness
| File | Summary | Severity |
|------|---------|----------|
| [camera_failure_panics.md](camera_failure_panics.md) | `.expect`/`.unwrap` on camera paths panic app/thread; errors never reach the UI | Med-high |
| [cv_worker_model_mutex.md](cv_worker_model_mutex.md) | Model mutex held for worker thread lifetime; latent deadlock on reload/double-start | Medium |
| [close_to_tray_behavior.md](close_to_tray_behavior.md) | ✕ quits mid-tracking (loses unflushed session log); should hide to tray | Medium |

## Structure & idiomatic Rust
| File | Summary | Severity |
|------|---------|----------|
| [refactor_update_cvinference_arm.md](refactor_update_cvinference_arm.md) | 115-line update arm; bad-posture predicate ×3, alert-open logic ×2, model-load ×2 | Medium |
| [dedupe_metrics_log_parsing.md](dedupe_metrics_log_parsing.md) | `load_today` duplicates `parse_log_totals` line-for-line | Low |
| [replace_polling_with_blocking.md](replace_polling_with_blocking.md) | 5 ms frame poll + 100 ms tray poll → watch channel / event handler | Low-med |
| [minor_idiomatic_cleanups.md](minor_idiomatic_cleanups.md) | Grouped small items: needless Box, stringly enums, dup constant, dead `InfType`, etc. | Low |

## UI / cross-platform
| File                                                             | Summary                                                                                   | Severity     |
| ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------- | ------------ |
| ~~[responsive_main_window.md](responsive_main_window.md)~~       | ~~Fixed 1206×961 non-resizable window fails on 768p / 150 % scaling; fixed-width panels~~ | ~~**High**~~ |
| [alert_window_cross_platform.md](alert_window_cross_platform.md) | Borderless+maximize behaves differently per WM/OS; fixed 72 px type; no keyboard dismiss  | Medium       |
| [icon_glyphs_cross_platform.md](icon_glyphs_cross_platform.md)   | Unicode-glyph icons depend on per-OS font fallback; bundle fonts/icon set                 | Medium       |
| ~~[consolidate_theme_tokens.md](consolidate_theme_tokens.md)~~   | ~~Hardcoded copies of ELEV/HOVER/PANEL tokens; legacy alias migration~~                   | ~~Medium~~   |
| [restyle_settings_panel.md](restyle_settings_panel.md)           | Settings + first-run prompt still on legacy style, off the "Refined Slate" kit            | Medium       |
| [tray_icon_cross_platform.md](tray_icon_cross_platform.md)       | Procedural placeholder icon; macOS template image; status-colored icon idea               | Low-med      |
| [quality_metric_empty_state.md](quality_metric_empty_state.md)   | "100% quality" with zero data; inconsistent `--`/`0s` empty states                        | Low          |

## Suggested ordering
1. ~~`cv_buffer_pool_leak` (leak) and `camera_failure_panics` (crashes) — user-visible damage.~~
2. `responsive_main_window` — blocks the cross-platform release story.
3. `eliminate_frame_clones` + `refactor_update_cvinference_arm` — touch the same hot
   paths; doing them together avoids rebasing.
4. Theme/settings/glyph trio (`consolidate_theme_tokens`, `restyle_settings_panel`,
   `icon_glyphs_cross_platform`) — one visual-polish branch.
5. Everything else opportunistically.
