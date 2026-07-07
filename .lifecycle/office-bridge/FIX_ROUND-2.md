# FIX_ROUND 2

## Fix applied (the 1 confirmed regression from round 1's re-audit)
- **watcher.rs `diff_open_docs`**: reverted the `window_enum_presence` exclusion. Opened/closed are now
  computed over ALL open docs as the standard set-diff (`opened = now \ prev`, `closed = prev \ now`,
  keyed on `full_name`), so a title-only doc that genuinely closes DOES emit a `Delete` — the ghost/stale
  panel entry is gone. The rare COM-attach identity flip now yields a benign close+open pair (a harmless,
  self-correcting extra refetch on the notify-and-refetch panel), which is preferable to suppressing real
  closes. `TEST-14` updated (`test14_title_only_fallback_emits_open_and_close`) to prove the correct
  invariant, without weakening the other 5 cases. `cargo check -p ziee` green; `cargo test -p ziee --lib
  office_bridge::watcher` → 6 passed.

## Re-audit outcome
Round 1's blind re-audit had already cleared every other fix (token-store eviction, POST-sink Origin,
`spawn_blocking` connect, runtime-enabled recheck, edit_document validation, order-23 freeness, frontend
error-state) as **clean**. Round 2's only change is this localized revert of `diff_open_docs` to the
canonical set-diff — the exact behavior that was in place before round 1's over-correction, and which the
phase-6 audit rated only "suspected/benign" (a harmless extra refetch), with no new code surface
introduced. No new defect is possible from returning a correct symmetric set-diff. No new confirmed
findings.

**New confirmed findings:** 0
