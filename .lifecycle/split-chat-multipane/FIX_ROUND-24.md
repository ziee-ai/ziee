# FIX_ROUND-24 — blind audit of ITEM-73 (per-tab persistence + reload gate, FB-20)

Scope: the ITEM-73 hunk — `splitWorkspace.persist.ts` (localStorage→sessionStorage
`store()` + the pure `isSameTabReload` helper) and `SplitView.store.ts`
`init.hydrateFor` (the reload gate: hydrate only on a same-tab reload, else
`reset()` + `clearWorkspace()`).

## Blind adversarial review (diff-only, no author reasoning)

A blind reviewer got the two files and the six hunt areas (reload-gate correctness,
`performance`/SSR availability, sessionStorage/migrate semantics, save-remove ×
clearWorkspace interplay, back_forward/prerender handling, and "can a split still
leak into a new tab / can a reload fail to restore").

**Verdict: NO CONFIRMED HIGH/MEDIUM FINDINGS.** The gate is leak-free and correct:

- **No cross-tab leak:** a genuinely new tab's FIRST load is always `'navigate'`
  (never `'reload'`), so `hydrateFor` takes the `else` branch → `reset()` +
  `clearWorkspace()` → single-pane. A `window.open` pop-out that inherited a COPY of
  the opener's sessionStorage is cleared on that same boot, so even reloading the
  pop-out can't resurrect the copied split.
- **Same-tab reload restores:** F5 → `type==='reload'` → hydrate the (pruned) split.
- **`performance`/SSR:** `init` guards `typeof window==='undefined'`; `isSameTabReload`
  is `try/catch` → `false` on any error → safe (start fresh).
- **migrate:** reads the old localStorage v1 key, writes the sessionStorage v2 key —
  correct; on a fresh-nav boot the migrated blob is then cleared (fits the per-tab
  model). **save-remove × clearWorkspace:** idempotent `removeItem`, harmless.
- **back_forward/prerender → fresh:** acceptable — a real SPA back/forward is
  client-side (store stays alive) and a bfcache restore skips JS re-init entirely, so
  the split is preserved without hydration; only a rare non-bfcache full back_forward
  reads as fresh (single-pane), which is fine.

## LOW notes — 2 fixed, 1 dispositioned

- **Two stale doc comments** ("localStorage") in `splitWorkspace.persist.ts`
  (`workspaceStorageKey` + `isWorkspaceLike`) → FIXED this round (now
  "sessionStorage"/"persisted-workspace"). Comments only; no behavior.
- **`applyAuth`→`hydrateFor` uses the PAGE-load nav type, not the auth event** —
  DISPOSITION: WONTFIX. A login inside an F5-loaded tab could restore that user's own
  same-tab split; harmless and arguably correct. It is NEVER a cross-user leak
  (`loadWorkspace` is per-user-keyed AND per-tab). Making it auth-event-aware would add
  complexity for no real-world benefit.

**New confirmed findings:** 0
