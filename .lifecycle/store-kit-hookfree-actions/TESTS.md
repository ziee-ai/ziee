# TESTS — store-kit-hookfree-actions

Tiers mirror the repo: unit = co-located `*.test.ts` run by `test:unit`
(`node --test` + the ITEM-10 loader); e2e = Playwright specs under
`src-app/ui/tests/e2e`. The proxy logic, React, and zustand are REAL in every
unit test — only the two unrelated `@/core/{module-system,events}` boundaries are
stubbed (they are never called by the proxy factory under test).

## Tests

- **TEST-1** (tier: unit) [covers: ITEM-4, ITEM-10] file: `src-app/ui/src/core/stores.test.ts` — asserts: an action on a raw zustand store, read through `createStoreProxy`, is returned resolved from `getState()` and is callable OUTSIDE any React render with NO hook violation, mutating state; the real proxy module loads under `node --test` via the loader (proves ITEM-10).
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/core/stores.test.ts` — asserts: `proxy.$` returns the `getState()` snapshot hook-free outside render, and `proxy.$.field === store.getState().field`.
- **TEST-3** (tier: unit) [covers: ITEM-1, ITEM-2] file: `src-app/ui/src/core/stores.test.ts` — asserts: reading a non-function state VALUE (`proxy.field`) outside render still throws a hook violation — the render-only reactive contract is preserved and `$` is the required handler-side escape.
- **TEST-4** (tier: unit) [covers: ITEM-1, ITEM-2] file: `src-app/ui/src/core/stores.test.ts` — asserts: `proxy.__state` is NO LONGER a special hook-free snapshot — accessing it outside render throws exactly like any reactive read (proving the alias was removed at runtime, not merely renamed).
- **TEST-5** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/core/store-kit.test.ts` — asserts: an action authored via `defineStore({ actions })` lands as a function on `getState()` and is callable hook-free through the proxy — confirming the function-detection is correct for the store-kit authoring model.
- **TEST-6** (tier: unit) [covers: ITEM-3, ITEM-4] file: `src-app/ui/src/core/store-kit.test.ts` — asserts: for a `defineLocalStore(...).use()` component rendered via `react-dom/server`, a reactive field read works in render, `$` snapshot is correct, the action is callable from a captured-handler context without a hook violation, and `__state` is not a hook-free snapshot on the local proxy.
- **TEST-7** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/chat/conversation-list-search.spec.ts` — asserts: typing in the conversation-list search box drives the swept `Stores.ChatHistory.setSearchQuery` / `loadConversations` handlers (formerly `.__state.*`) and filters the list — a direct action call from a real onChange handler works end-to-end.
- **TEST-8** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/projects/conversation-list-interaction.spec.ts` — asserts: deleting a conversation drives the swept `Stores.ChatHistory.deleteConversation` handler from a real onClick and removes the row from the list.
- **TEST-9** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/core/store-kit.test.ts` — asserts: `proxy.$.__destroy__` is reachable hook-free and calling `proxy.$.__destroy__()` runs the teardown without triggering store `__init__` — the exact access pattern the HMR-destroy path in `module-system/store.ts` now uses.
- **TEST-10** (tier: unit) [covers: ITEM-7] file: `src-app/ui/src/core/stores.test.ts` — asserts: the shared proxy contract that the desktop workspace consumes via `@ziee/ui-core` exposes only `$` (plus `__setState`/`__ref*`/`__destroyed`) as hook-free specials and NO `__state` — the invariant the desktop swept sites depend on; desktop sweep completeness is additionally tsc-backstopped by `npm run check (desktop/ui)`.
- **TEST-11** (tier: unit) [covers: ITEM-8] file: `src-app/ui/biome-plugins/no-store-internal-state.grit` — asserts: `biome lint` with the plugin flags a `.__state` member-access fixture as an error, and the clean post-sweep tree passes `npm run lint:guardrails` in both workspaces (the guardrail bans reintroduction).
- **TEST-12** (tier: unit) [covers: ITEM-9] file: `src-app/ui/scripts/gen-state-matrix.mjs` — asserts: after `npm run gen:state-matrix`, `npm run check:state-matrix` reports no drift in BOTH workspaces — the regenerated `stateMatrix.generated.ts` + `STATE_MATRIX.md` match the swept `.$.` source.

## Coverage map (every ITEM ≥1 TEST)

- ITEM-1 → TEST-2, TEST-3, TEST-4
- ITEM-2 → TEST-3, TEST-4
- ITEM-3 → TEST-6
- ITEM-4 → TEST-1, TEST-5, TEST-6
- ITEM-5 → TEST-9
- ITEM-6 → TEST-7, TEST-8
- ITEM-7 → TEST-10
- ITEM-8 → TEST-11
- ITEM-9 → TEST-12
- ITEM-10 → TEST-1

UI diff → ≥1 `tier: e2e` present (TEST-7, TEST-8). ✓
