# DRIFT-14 — ITEM-73 (per-tab persistence) changes the ITEM-25/26 full-load model

Reconciling the ITEM-73 / DEC-74 implementation against the PLAN behavior that
ITEM-25 (URL→workspace reconcile) + ITEM-26 (workspace persistence) documented.

- **DRIFT-14.1** — verdict: impl-wins — **The workspace is per-TAB (sessionStorage),
  not per-user (localStorage).** ITEM-26 originally persisted to a per-user
  localStorage key shared by every tab. That sharing WAS the FB-20 bug (a new tab
  restored another tab's split). DEC-74 (human-picked "Both") moves it to
  sessionStorage. PLAN/ITEM-26 amended to the per-tab model; the accepted trade-off
  (no cross-browser-session restore) is recorded in DEC-74.

- **DRIFT-14.2** — verdict: impl-wins — **A full-load navigation no longer restores
  the split; only a same-tab RELOAD does.** ITEM-25 described the URL as a "view into"
  the workspace that reconciles on every full load (focus an existing pane / replace
  the focused pane). Under the reload gate a fresh full-load navigation (new tab,
  ⤢ pop-out, deep link, address-bar nav) is URL-AUTHORITATIVE → single-pane; the
  URL→workspace reconcile now runs ONLY for an IN-MEMORY split (SPA nav, panes≥2 still
  mounted). This is REQUIRED to stop cross-tab inheritance (a `window.open` child
  inherits a COPY of the opener's sessionStorage) and is also the more intuitive
  deep-link behavior (a URL opens that conversation, not a resurrected split). The
  ITEM-25 reconcile itself is unchanged for the SPA-nav path it still serves
  (`open-in-split.spec.ts` + the ITEM-43 open-choice prompt TEST-63).

- **DRIFT-14.3** — verdict: resolved — **The specs that encoded the OLD full-load
  model were updated, not deleted to go green.** `workspace-persist-nav.spec.ts`
  (TEST-51) rewritten to assert reload-restores + fresh-nav-single-pane;
  `persistence.spec.ts` comment corrected to sessionStorage (its reload-restore
  assertion is unchanged and still passes); `header-back-visibility.spec.ts` clears
  sessionStorage (and the fresh `goto` to the pop-out route already resets). Each
  change reflects the DEC-74 model, is traceable to it, and is covered by the new
  TEST-110/111.

**Unresolved drifts:** 0
