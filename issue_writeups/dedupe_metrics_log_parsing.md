# `MetricsStore::load_today` duplicates `parse_log_totals` line-for-line

**Severity: low — pure duplication, ~45 lines.**

## Where
`src/metrics.rs`:
- `load_today` (~lines 420–465)
- free function `parse_log_totals` (~lines 576–625)

## Problem
Both functions open a log file, iterate lines, `splitn(2, ',')`, parse the timestamp, and
run the identical `Start/Stop/GoodToBad/BadToGood` state machine accumulating
`(breaks, bad_secs, tracked_secs)`. The only difference: `load_today` writes into
`self.breaks_today` / `self.bad_posture_secs_today` / `self.tracked_secs_today` fields
directly instead of returning a tuple.

If the log format ever changes (new event type, different separator), someone will fix
one copy and miss the other — and the bug would be subtle (all-time totals diverging from
daily totals for the same file).

## Fix
`load_today` becomes:
```rust
fn load_today(&mut self) {
    let (breaks, bad, tracked) = parse_log_totals(&self.log_path());
    self.breaks_today += breaks;
    self.bad_posture_secs_today += bad;
    self.tracked_secs_today += tracked;
}
```
(`+=` vs `=` is equivalent here since it runs once from `new()` on zeroed fields.)

One behavioral nuance to verify: `load_today` tracks `last_start_ms` but never
*uses* an unmatched trailing `Start` (a crash mid-session leaves a `Start` with no
`Stop`; both implementations silently drop that open interval). That's identical
behavior in both copies today, so the consolidation is safe — but it's worth a comment,
and possibly a follow-up to recover crashed-session time using the file's last event
timestamp.

## Clues
- `parse_log_totals` already has good test coverage (`parse_log_totals_*` tests at the
  bottom of `metrics.rs`); after deduplication those tests cover the startup-restore path
  too. Consider adding one integration-style test that builds a store pointed at a
  tempdir containing a prewritten today-log (the existing `empty_store` helper bypasses
  `new()`, so a new test would need `MetricsStore::new`-like wiring with a temp
  `data_dir` — may require making `data_dir` injectable, which is itself a nice
  testability win).
