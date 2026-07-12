# PLAN_AUDIT — plan vs codebase

## Breakage risk

- The `complete` handler is the single normal-completion path; reordering it must preserve every
  side effect it performs today: `afterStreamComplete` extension hook, `computeForkPoints`,
  `branchChangedDuringStream` reset, `clearConversationCache` for background convos, and the
  cancelled → `lastTurnInterrupted` mapping. The atomic rewrite keeps ALL of these; only the
  ordering (teardown moves AFTER the persisted fetch) changes.
- `reconcileTail` is ALSO called from the `started` receiving-device path (~L1364). We do NOT change
  `reconcileTail` (we inline getHistory + `finalizeTailWindow` in `complete`), so that path is
  untouched — no risk to cross-device catch-up.
- The "generating" affordance / stop button reads `isStreaming` + `streamingMessageId`. Moving their
  clear to after the getHistory means they persist for one extra fast round-trip. Acceptable and
  arguably more correct (affordance stays until the finalized turn is on screen). Mid-stream cancel
  still arrives as a `complete{finish_reason:'cancelled'}` → same atomic path → still clears.
- getHistory failure in the new path must not strand `isStreaming:true` forever, and must keep the
  streamed row visible. Covered by ITEM-7 (catch clears flags, leaves the row).

## Pattern conformance

- `finalizeTailWindow` mirrors `mergeTailWindow`/`appendWindow` (pure, Map-in/Map-out, doc-comment,
  node:test) — conforms to `messageWindow.ts`.
- The `!finalizing` conjunct mirrors the existing `!isStreaming`/`!interrupted` conjuncts in
  `emptyCompletion.ts`; the prop threading mirrors `isStreaming`/`interrupted` in `MessageList.tsx`.
- Store flag `finalizingTurn` mirrors `branchChangedDuringStream` (transient boolean, initial false,
  not persisted in the snapshot — it is sub-second and self-clears).

## Migration collisions

None. No migration added (frontend-only).

## OpenAPI regen

None. No backend type change; `openapi.json` / `api-client/types.ts` untouched. Therefore this is
NOT treated as a backend diff by the gates.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — reorders the existing `complete` handler; keeps all side-effects;
  snap-to-tail branch replicates `loadMessages` (verified L826-848 equivalent).
- **ITEM-2** — verdict: PASS — additive pure helper next to `mergeTailWindow`; no caller churn.
- **ITEM-3** — verdict: PASS — new transient boolean mirrors `branchChangedDuringStream`; threading
  mirrors `isStreaming`. Not added to the snapshot interface (transient) — safe default false.
- **ITEM-4** — verdict: PASS — additive conjunct; `emptyCompletion.test.ts` matrix extended.
- **ITEM-5** — verdict: PASS — the genuinely-empty path leaves `finalizing` false once finalized,
  so `empty-completion.spec.ts` (post-complete + post-reload notice) stays green.
- **ITEM-6** — verdict: CONCERN — approval-scroll code change is CONTINGENT; the base behavior may
  already be correct once the remount is gone. Resolve by live/e2e observation in Phase 8; if no
  residual race, ITEM-6 ships as a verify-only item (its e2e assertion still covers it) with no
  `ConversationPage.tsx` change. Recorded as DEC-4.
- **ITEM-7** — verdict: PASS — error/cancel/background/reset null-sites are left byte-identical
  except the on-screen `complete` path; regression asserted by keeping existing behavior + new
  fallback.
