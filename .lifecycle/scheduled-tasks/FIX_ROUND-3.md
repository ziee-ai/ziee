# FIX_ROUND-3 — Round 2 (Follow-up & Series), fix pass 1

Merged the Phase-6 blind-audit ledger (3 blind agents: backend-correctness, backend-security,
frontend) and fixed every confirmed finding, then re-ran a blind round.

## Fixed (9 Phase-6 findings)
1. **repository.rs — page i64 overflow (medium):** `offset` now uses saturating math
   (`page.max(1).saturating_sub(1).saturating_mul(per_page)`) so a crafted huge `page`
   yields an empty page, never a panic/500.
2. **repository.rs — no ORDER BY tie-breaker (low):** added `id DESC` after `fired_at DESC`
   so equal-timestamp runs can't be duplicated/omitted across pages.
3. **continue_chat.rs — series seed uncapped (low):** the aggregate assistant text is now
   capped at `SERIES_TEXT_MAX = 12000`.
4. **ScheduledTasksPage.tsx — full-page reload nav (medium):** replaced `window.location.href`
   with react-router `useNavigate` across RunRow / SeriesChooser / TaskRow.
5. **ScheduledTasksPage.tsx — dead page-size selector (medium):** `loadRuns` gained a `perPage`
   arg; `onPageSizeChange` now honors the chosen size.
6. **runTimeline.ts — duplicate series option (low):** `seriesChoices` now dedupes by value.
7. **ScheduledTasksPage.tsx — pruned-page strand (low):** the empty-state render now keys off
   `total === 0` (an empty NON-first page keeps the panel + pager).
8. **ScheduledTasksPage.tsx — Select-as-action (low):** `SeriesChooser` is now a `Dropdown`
   action menu, matching the file's established action primitive.
9. **ScheduledTasksPage.tsx — expand a11y (low):** the expand button gained `aria-controls`
   to the detail region + a visible chevron affordance.

Also fixed a test bug found while running: TEST-43 passed `"paged"` as `seed_prompt_task`'s
`kind` arg (must be `once`/`recurring`) → schedule-coherence constraint violation; corrected to
`"recurring"`.

## Re-audit (blind round)
A fresh blind agent reviewed the post-fix worktree and confirmed all 9 fixes correct, finding
ONE residual defect:

- **state-management (low):** the `total === 0` fix (#7) still strands the user when prune drops
  `total` to `<= perPage` while `meta.page >= 2` and a sync reload refetches that out-of-range
  page — 0 rows render with the pager hidden (`total <= perPage`).

**New confirmed findings:** 1
