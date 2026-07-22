# TESTS — ui-batch

Every PLAN ITEM is covered; nothing is `[DESCOPED]`.

**Tier rationale.** `npm run test:unit` is `node --import ./scripts/node-test-loader.mjs
--test "src/**/*.test.ts"` — the plain Node test runner with **no jsdom**, so a
React component cannot be mounted at the unit tier in this repo. Everything that
is genuinely a RENDER or LAYOUT claim is therefore proven at `tier: e2e`, per B7
("verification means RUNNING it"). The gallery-driven visual specs
(`tests/e2e/visual/`, `playwright.visual.config.ts`) run backend-free against the
component gallery and are the repo's established vehicle for geometry assertions
— modelled on `tests/e2e/visual/chat-collapse-borders.spec.ts` (issue #183), which
asserts a layout EFFECT rather than a class string, precisely so an equivalent
re-implementation cannot fail spuriously.

**Anti-inflation note.** The pure reconcile branches this feature depends on are
ALREADY covered by `reconcile.test.ts` (`"auto while split → replaces the focused
pane"` at :118 — the bug precondition — and `"auto with an empty workspace →
navigate, state unchanged"` at :148 — the fixed state). Re-enumerating those as
new TEST-IDs would be coverage inflation, so they are cited as existing context
and TEST-1 instead asserts the thing no existing test does: that `reset()`
COMPOSES with the reducer to move the workspace from the first branch to the
second.

## Tests

- **TEST-1** (tier: unit) [covers: ITEM-7] file: `src-app/ui/src/modules/chat/core/stores/SplitView.store.test.ts` — asserts: from a real 2-pane split, `reset()` leaves `panes: []` / `focusedPaneId: null`, and a subsequent `openConversationInWorkspace(newId, 'auto')` returns outcome `navigate` (NOT `replace`) with no pane resurrected — the exact store-level transition the `/chat` route now performs, proving `reset()` and the reducer compose. Distinct from the existing `'setMode toggles split/tabs; reset clears the layout'` case, which only checks the cleared fields and never drives an open through the reducer afterwards.
- **TEST-2** (tier: e2e) [covers: ITEM-1] file: `src-app/ui/tests/e2e/visual/composer-model-selector.spec.ts` — asserts: at a WIDE composer, a long model name ("GPT OSS 120B …") renders IN FULL — the label's `scrollWidth <= clientWidth` (not truncated) and its text equals the seeded name — and the trigger's border box is wider than the old 130px cap yet within the 20rem soft ceiling. This is what proves the trigger is content-sized rather than merely re-pinned to a bigger number.
- **TEST-3** (tier: e2e) [covers: ITEM-2] file: `src-app/ui/tests/e2e/visual/composer-model-selector.spec.ts` — asserts: a model name PAST the trigger's soft ceiling is genuinely ellipsized — `getComputedStyle(label).textOverflow === 'ellipsis'`, `display === 'block'` (a flex item renders the ellipsis inert — the original defect) AND `scrollWidth > clientWidth` — the trigger stays within the ceiling, and the label's right edge stays inside the trigger's content box (no spill over the chevron). All conditions are required: the ellipsis property alone is inert without the clip, which is exactly the pre-fix state. (AMENDED per DRIFT-1.4: originally "the same name at a narrow viewport". Measured — a name that FITS never truncates at any real viewport, because the composer's left group absorbs the pressure first; the ellipsis path is reachable only past the ceiling, which is precisely when it should engage.)
- **TEST-4** (tier: e2e) [covers: ITEM-3] file: `src-app/ui/tests/e2e/visual/composer-model-selector.spec.ts` — asserts: the Send button's rendered width is IDENTICAL at the wide and narrow composer widths and its border box stays fully inside the composer, while the model trigger's width shrinks between the two — i.e. the model name is what yielded and Send did not. Measured across the same two viewports TEST-2/TEST-3 use, so the "who gave way" claim is proven by comparison rather than asserted.
- **TEST-5** (tier: e2e) [covers: ITEM-2] file: `src-app/ui/tests/e2e/visual/composer-model-selector.spec.ts` — asserts: with the dropdown OPEN at the narrow width (where the trigger is ellipsized), the selected option's row renders the model's FULL text untruncated (`scrollWidth <= clientWidth`), so the truncated trigger never leaves the user unable to read the name. Guards the `popupMatchSelectWidth={false}` behavior the fix depends on.
- **TEST-6** (tier: e2e) [covers: ITEM-4, ITEM-5, ITEM-6] file: `src-app/ui/tests/e2e/layouts/sidebar-title-alignment.spec.ts` — asserts: the "Navigation", "Tools" and "Recent chats" captions report the SAME content-box left edge (within sub-pixel tolerance) in the REAL app shell. Three captions converging on one edge is only achievable through the single shared component, so this is the covering assertion for ITEM-4 as well as the two call sites. (AMENDED per DRIFT-1.3: planned as a gallery visual spec, but the gallery's `PageFrame` mounts a route element WITHOUT its layout, so the real `LeftSidebar` and its slot-populated sections never render there. Same tier, run against the real shell instead — B7.)
- **TEST-7** (tier: e2e) [covers: ITEM-5, ITEM-6] file: `src-app/ui/tests/e2e/layouts/sidebar-title-alignment.spec.ts` — asserts: every sidebar menu ROW (primary actions, navigation, tools, recent-chat rows) still shares ONE left edge with the others, and that edge is strictly GREATER than the caption edge. This is the anti-cheat control: it makes "align the captions by dragging the rows around" fail, and pins the intended relationship (captions hang left of their rows, as "Recent chats" already did) — including against the opposite over-correction of pushing the captions right to meet the rows.
- **TEST-8** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/14-split-chat/new-chat-collapses-split.spec.ts` — asserts: the reported repro EXACTLY (B9) — open a 2-pane split, click the sidebar "New Chat", type a message and send — ends on a SINGLE-pane view of the newly created conversation: `split-chat-view` is absent, exactly one conversation surface is rendered, the URL points at the new conversation, and neither of the two original conversations is on screen. Fails on `main` (the split reappears with the new chat wedged into the focused pane).
- **TEST-10** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/14-split-chat/project-new-chat-collapses-split.spec.ts` — asserts: the same collapse through the OTHER surface that reproduced the bug — build a 2-pane split, then create a conversation from a project detail page's inline composer — ends on a single-pane view of the new conversation at `/projects/:id/chat/:cid` (which renders the same `ConversationPage`), with `split-chat-view` absent and exactly one composer. Added after a blind audit found `ProjectDetailPage` was a structural twin of `NewChatPage` reproducing the identical hijack, and that its fix had ZERO coverage at any tier — deleting it left the unit test, TEST-8 and every gate green.
- **TEST-9** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/14-split-chat/new-chat-adopt.spec.ts` — asserts: (EXISTING spec, run as the paired regression control) creating a chat from INSIDE a split pane via `ConversationPickerPane` still adopts into that pane — the split survives, both panes remain, and the other pane is undisturbed. Without this, TEST-8 could be satisfied by collapsing the split on every new-chat path, which would destroy the in-pane flow. The two together pin both sides of the boundary.

## Coverage map

| ITEM | Covered by |
|---|---|
| ITEM-1 (content-sized trigger) | TEST-2 |
| ITEM-2 (real ellipsis) | TEST-3, TEST-5 |
| ITEM-3 (Send never yields) | TEST-4 |
| ITEM-4 (shared title component) | TEST-6 |
| ITEM-5 (Navigation/Tools captions) | TEST-6, TEST-7 |
| ITEM-6 (Recent chats caption) | TEST-6, TEST-7 |
| ITEM-7 (`/chat` collapses the split) | TEST-1, TEST-8, TEST-9 |

Frontend diff ⇒ `tier: e2e` present (TEST-2…TEST-9). ✔

## Permission gating (A9 / A10)

**Not applicable, verified rather than assumed.** This diff introduces no
permission: no `modules/*/permissions.rs` is touched, no migration is added, and
no `Permissions.*` constant is created. The surfaces changed are already gated
upstream and unchanged by this work (`LeftSidebar.tsx:85-87` filters widget slots
through `isAllowed`; the model selector renders inside the composer, which is
reached only via an authenticated chat route). So no `[negative-perm]`
restricted-user e2e is required for this feature.

## Supporting fixtures (not tests themselves)

- TWO gallery deep-states in `src-app/ui/src/modules/chat/gallery.tsx` seeding
  `ModelPicker`, siblings to the existing `deep-chat-empty-model-picker` seed —
  required so TEST-2…TEST-5 can render both regimes backend-free.
  `deep-chat-long-model-name` carries a name that FITS inside the soft ceiling
  (TEST-2); `deep-chat-overlong-model-name` carries one past it (TEST-3/4/5).
  Both share one `holdSingleModel` helper.
- TEST-8 reuses the real provider/model setup helpers already used by
  `new-chat-adopt.spec.ts` (`createProviderViaAPI` / `createModelViaAPI` /
  `assignProviderToAdministratorsGroup`); no shared harness file is modified (B3).
