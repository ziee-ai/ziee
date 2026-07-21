# FIX_ROUND-4 — workflow-kind-agent

Round-4 blind re-audit of the round-3 keyed-lock + recovery-sweep change. It VERIFIED the core correct:
the keyed lock serializes (same Arc per id, no shard-guard held across `.await`, `remove_if(strong_count==1)`
can't race a waiter in, guard releases on panic/future-drop), the recovery sweep runs before this op's own
staging is created, and no 200 is returned on a disk/DB divergence. It found **1 new confirmed finding
(0 high, 0 medium, 1 low)**, now fixed.

## New finding this round + resolution
- **R4-1 (LOW) — recovery could delete the real bundle in an ambiguous multi-`.old-` state.** When
  `bundle_root` is missing and MORE THAN ONE `.old-*` survives (only reachable via a prior failed
  `remove_dir_all` + a crash mid-swap), the newest-by-mtime promotion is arbitrary on an mtime tie /
  unreadable mtime, and the non-promoted `.old-*` was then unconditionally deleted — risking destruction
  of the true last-live bundle. **Fixed:** the sweep now preserves ALL remaining `.old-*` whenever it had
  to recover (live bundle was missing at sweep entry); it deletes `.old-*` ONLY when the live bundle was
  present all along (confidently stale). `.staging-*` are always safe to prune. The change is
  strictly-fewer-deletions in the recovery branch, so it cannot introduce a new defect.

## Build after round-4 fix
cargo check `-p ziee` clean.

**New confirmed findings:** 1
