# Posture quality shows "100%" with zero data; empty states inconsistent across cards

**Severity: low — misleading first impression and inconsistent dashboard semantics.**

## Where
- `src/metrics.rs` `posture_quality_today` / `posture_quality_session`
  (~lines 221 & 252): both `return 1.0` when `tracked <= 0.0`.
- `src/app/components/metrics_panel.rs`:
  - `view_quick` (~line 355) renders that as "POSTURE QUALITY 100%" with a full green
    progress bar before the user has ever tracked.
  - `view_session` handles the same situation differently: it shows "--" for session
    length when inactive but still renders quality as 100% and "Breaks 0".
  - `view_daily`/`view_all_time` show "0s" totals rather than an empty-state hint.

## Problem
"100% quality" is an earned-looking number for *no data*. A new user sees a perfect
score, which both undermines trust ("it's not measuring anything") and removes the
incentive to start a session. The dashboard also mixes three different empty
representations: `--`, `0s`, and `100%`.

## Suggested direction
1. Change the getters to return `Option<f32>` (None when `tracked == 0`); the test
   `posture_quality_today_is_one_when_untracked` (metrics.rs tests) encodes the current
   behavior and should flip to expect `None`.
2. In `metrics_panel`, render `None` as "--" with the neutral `OWHITE`→`T1` color and no
   progress bar fill — consistent with `fmt_duration(None)`'s existing "--" convention.
3. Decide one rule for all cards: untracked values show "--" (not 0), and apply it to
   breaks/bad-time cards when `tracked_duration_*` is zero. Small helper like
   `fn dash_if_untracked(tracked: Duration, value: String) -> String` keeps it tidy.

## Clues
- `posture_quality_*` is also consumed by the QuickView color thresholds
  (`view_quick`, quality ≥ 0.8 green / ≥ 0.5 white / else red) — a `None` arm slots in
  naturally there.
- Grep for `posture_quality` to find all call sites; there are only the two panels.
- Don't change the *clamp* behavior for the >0 case; the clamp tests
  (`posture_quality_today_clamps_when_bad_exceeds_tracked`) still apply.
