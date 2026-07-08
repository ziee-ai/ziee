# message-scroll-perf — PLAN_AUDIT

Audit of the plan against the codebase at `origin/main` (`a101c851`).

## Breakage risk

- **ITEM-1 (estimateSize)** — `estimateSize` is called by the virtualizer for every
  index; swapping the constant for `estimateMessageHeight(messagesArray[i], width)`
  is internal to `MessageList.tsx`. `getItemKey` is already by message id, so the
  measurement cache keys don't change. No external caller of `MessageList` sees a
  behavior change. Low risk. The estimator must tolerate a missing/undefined
  message (index transiently out of range mid-window-reset) and return the old
  constant as a floor — the current `estimateSize: () => 140` never throws, so the
  replacement must be equally total.
- **ITEM-2 (initialMeasurementsCache + write-back)** — `initialMeasurementsCache`
  is a supported `useVirtualizer` option in the installed `@tanstack/react-virtual`
  **3.14.5** (confirmed in `virtual-core` dist). It seeds `measurementsCache` at
  construction only; a wrong-width stale height would otherwise persist, so the
  width-bucket key is load-bearing (mitigated in DEC). Writing back measured
  heights must read the virtualizer's own measurements (not re-measure the DOM) to
  avoid a second observer. Risk: an over-eager `onChange` write-back that runs every
  scroll frame — must write only changed, settled heights. Medium risk; covered by
  DEC-2 + TEST-2.
- **ITEM-3 (ReservedImage)** — the current `img` override enforces a security
  policy (blocks external/`data:` src → `BlockedImage`). `ReservedImage` must be
  inserted ONLY on the allowed-src branch (same-origin / `/`-rooted), preserving
  the `BlockedImage` path byte-for-byte. Regressing image SSRF/exfil blocking is
  the real hazard here — the audit (phase 6, security angle) must confirm the
  policy branches are untouched. Medium risk; DEC-3 pins that the security check
  stays first and `ReservedImage` only wraps an already-permitted `<img>`.
- **ITEM-4 (definite height)** — `MarkdownTable` already caps at
  `max-h-[min(60vh,36rem)]`; inline CSV already fixed (see spec). This item is
  mostly *verification*; any added bound is additive CSS on an inline preview
  wrapper. Low risk.
- **ITEM-5 (overscan)** — a single numeric constant; the only risk is pop-in if set
  too low. Locked by measurement in DEC-5. Low risk.
- **ITEM-6 (anchor reconcile)** — HIGHEST risk: it touches the just-shipped
  no-teleport prepend invariant (`lazy-load-messages.spec.ts` TEST-5 equivalent).
  The change must be guarded so `restoreAnchor` still pins the anchor; the existing
  e2e prepend invariant is the regression gate and must stay green. If measurement
  shows no residual double-adjust, ITEM-6 collapses to "no code change, keep the
  guard test" (recorded as a drift in phase 5). Medium-high risk; explicitly gated.
- **ITEM-7 (memo boundary)** — guard-only unless a real invalidation is found.
  Changing the find-context or `isStreaming` subscription shape could ripple to
  find/streaming behavior, so any change here re-runs `conversation-find.spec.ts`
  and the streaming specs. Low risk if it stays a test.

## Pattern conformance

- New pure utils (`estimateMessageHeight.ts`, `measuredHeightCache.ts`) with
  colocated `.test.ts` mirror the established `core/utils/scrollAnchor.utils.ts` +
  `core/stores/messageWindow.ts` pattern (pure function + Vitest unit). Conforms.
- `ReservedImage.tsx` under `components/common/` sits beside `BlockedImage.tsx`
  (same dir, same import site) — conforms to the existing markdown-override
  component placement.
- e2e specs under `tests/e2e/chat/` reuse `helpers/sse-mock-helpers.ts` and the
  `virtualize-messages.spec.ts` / `inline-csv-height-stability.spec.ts` idioms.
  Conforms; no new harness invented.
- Estimator/cache wiring extends the existing `useVirtualizer` block rather than
  restructuring `MessageList` — matches [[feedback_match_existing_patterns]].

## Migration collisions

None. This is a **frontend-only** change (no `src-app/server/migrations/**`
touched, no SQL). `ls migrations/` is irrelevant to this diff.

## OpenAPI regen

None. No Rust types, no `openapi.json`, no `api-client/types.ts` change — the fix
is entirely in `src-app/ui/src/**` render/scroll logic + tests. `just openapi-regen`
is **not** required, and the phase 3 / phase 8 frontend gates correctly treat this
as UI work (real `src-app/ui/**` source touched, not just generated files).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — internal to MessageList; `estimateSize` swap is
  total + cache-key-stable; estimator must be null-safe (noted).
- **ITEM-2** — verdict: PASS — `initialMeasurementsCache` exists in react-virtual
  3.14.5; width-bucket key + settled-only write-back required (DEC-2).
- **ITEM-3** — verdict: CONCERN — must preserve the existing image security policy
  branch exactly; `ReservedImage` wraps only already-permitted images. Resolved by
  DEC-3 (security check stays first) + phase-6 security angle. Not blocking.
- **ITEM-4** — verdict: PASS — mostly verification; `MarkdownTable` already capped;
  additive bound only where missing.
- **ITEM-5** — verdict: PASS — single constant; value fixed by measurement (DEC-5).
- **ITEM-6** — verdict: CONCERN — touches the just-shipped prepend no-teleport
  invariant; guarded by the existing prepend e2e as the regression gate, and may
  collapse to a no-op if measurement shows no double-adjust. Not blocking.
- **ITEM-7** — verdict: PASS — guard-only; code change only if a real scroll-time
  invalidation is found, and then re-runs find/streaming specs.

No `BLOCKED` verdicts. The two `CONCERN`s (ITEM-3 security-preservation, ITEM-6
anchor-invariant) are carried into DECISIONS + the phase-6 audit angles, not
resolved by amending the plan away.
