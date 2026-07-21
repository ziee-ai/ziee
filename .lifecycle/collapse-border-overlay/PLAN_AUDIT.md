# PLAN_AUDIT — collapse-border-overlay

Audited against the codebase at `origin/khoi` (`6ca93f123`) before writing code.

## Breakage risk

**Existing e2e that exercise the collapse path — both drive PURE-TEXT messages,
so the ITEM-3 split is a no-op for them. They are the guard that it STAYS a no-op:**

- `tests/e2e/chat/collapse-long-message.spec.ts` — asserts
  `collapsible-content` height ≤400px collapsed, grows >100px on expand,
  re-clamps. Its fixture (`LONG_TEXT`, 80 lines) is a single `text` block, so
  `splitTrailingProse` returns 0 and the whole message still goes inside
  `CollapsibleBlock`. **No behavior change.**
- `tests/e2e/visual/chat-scroll-stability.spec.ts` TEST-8/9 — `data-collapsed`
  persistence across virtualizer remount + in-place anchor, driven by
  `MessageListLongDemo` `g-msg-7` (`i % 7 === 0`), also a single `text` block.
  **No behavior change.**

**Genuine risks identified:**

1. **Empty trailing prose.** A turn ending on a tool card (no closing text) makes
   the prose suffix empty. `CollapsibleBlock` then wraps an empty div →
   `scrollHeight` 0 → `overflowing` false → no toggle rendered, no mask applied.
   Degrades correctly, but ITEM-3 should skip rendering `CollapsibleBlock`
   entirely in that case rather than emitting an empty wrapper into the DOM.
2. **`offerCollapse` is computed from the FULL message text**
   (`ChatMessage.tsx:56-65`, `messageText(message).length > 1200`) while the clamp
   now wraps only the trailing prose. A message that clears 1200 chars mostly via
   tool-call content but has short trailing prose will still take the
   `offerCollapse` branch, then measure no overflow and render no toggle. Correct
   outcome, one wasted wrapper div. Not worth re-scoping the threshold — the
   runtime overflow measurement is already the real gate by design
   (`collapsible.ts:5-10`).
3. **Virtualizer height estimates.** `estimateMessageHeight.ts:87-90` assigns
   tool/thinking blocks a flat `TOOL_ADD`; the estimator sums block heights and is
   agnostic to WHERE they render, so moving them out of the clamp does not change
   the ESTIMATE — but it does change the REAL height (previously capped at 384px,
   now cards + up-to-384px of prose). `chat-scroll-stability` TEST-6 (corrections
   ≤2 when settled) is the detector.

## Pattern conformance

- **Gallery deep-state (ITEM-1)** — `toolGroup` in
  `dev/gallery/fixtures/chat-deep.ts` is the reference bundle: `message()` /
  `block()` builders (verified at `:33-59`), registered in `chatDeepById` (`:466`)
  AND `CHAT_DEEP_CONVERSATION_IDS` (`:477`), then a `deepStates[]` entry in
  `modules/chat/gallery.tsx` (`~:194`) with `setup: () => whenLoaded(id)`.
  Interaction recipes follow `deep-chat-mcp-toolcall-error` (`:223`). Conforms.
  Note `gallery:check-fixtures` asserts fixture ids against the recording — the
  new bundle is synthetic (not recorded), matching every other `dee9000N-…` id,
  so it is unaffected.
- **Pure helper + `node:test` (ITEM-2)** — `shouldOfferCollapse` in
  `collapsible.ts` with `collapsible.test.ts` alongside is the exact sibling.
  Chat helpers use `node:test`, NOT Vitest (Vitest `include` is
  `src/**/*.store.test.ts` only). Conforms.
- **ITEM-3** conforms to the existing run-loop; no new abstraction, no reordering.

## Migration collisions

**None.** UI-only diff — no file added under `src-app/server/migrations/`, so
there is no migration number to collide. Highest existing migration is untouched.

## OpenAPI regen

**Not required.** No Rust type, route, or schema changes ⇒ neither
`openapi.json` nor `api-client/types.ts` is regenerated, and the diff is
correctly NOT classified as backend work. The desktop `ui/` counterpart
(R2-3) carries no override of `ChatMessage`/`CollapsibleBlock` — verified: the
chat module lives only in `src-app/ui/src/modules/chat/`.

## Item verdicts

- **ITEM-1** — verdict: PASS — mirrors the `toolGroup` bundle at
  `chat-deep.ts:466`; three registration sites confirmed (`chatDeepById`,
  `CHAT_DEEP_CONVERSATION_IDS`, `gallery.tsx` `deepStates`). Fixture must place
  the cards so at least one sits ABOVE 288px and one BELOW it, otherwise the
  surface cannot discriminate Mechanism A from Mechanism B.
- **ITEM-2** — verdict: PASS — pure function, no dependencies, sibling pattern
  and test runner both already present in the same directory.
- **ITEM-3** — verdict: CONCERN — two refinements required, neither blocking:
  (a) `collectToolRun` (`mcp/chat-extension/extension.tsx:252-257`) breaks on any
  non-`tool_use`/`tool_result` block, so a consumed run is ALWAYS purely
  structural today. Implement the kind-tagging defensively anyway — mark a node
  structural if **any** block it consumed is non-`text` — so a future
  `contentSpan` that spans text cannot silently mis-split.
  (b) Skip the `CollapsibleBlock` wrapper entirely when the prose suffix is empty
  (risk 1 above), rather than wrapping an empty div.
- **ITEM-4** — verdict: CONCERN — a bare `px-0.5` would inset the prose text by
  2px per side ON TOP of the parent's existing `px-0.5`, and gating it on
  `isClamped` would change text wrapping between collapsed and expanded (a visible
  reflow on every toggle). **Resolution: use `-mx-0.5 px-0.5` unconditionally** —
  equal negative margin + padding keeps the content box byte-identical in both
  states while pushing the `overflow-hidden` clip edge 2px outward, giving the
  ring room. The 4px growth is absorbed exactly by the parent's `px-0.5`, so the
  outer geometry is unchanged. Verify against the reproduction.
- **ITEM-5** — verdict: PASS — `gen:gallery-coverage`, `gen:state-matrix`,
  `gen:gallery-seed-registry`, `gen:testid-registry` all exist and are chained by
  `npm run check`; a new surface without them fails `check:state-matrix`.

No `BLOCKED` verdicts. The two `CONCERN`s are resolved in-plan above and carried
into DECISIONS.md (Phase 4) as DEC entries.
