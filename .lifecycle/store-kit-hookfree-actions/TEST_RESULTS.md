# TEST_RESULTS — store-kit-hookfree-actions

## Frontend gate (both UI workspaces touched)

npm run check (ui): PASS
npm run check (desktop/ui): PASS

(`npm run check` = tsc + biome guardrails incl. the new `no-store-internal-state`
grit + lint:colors/settings-field/… + check:kit-manifest/testid-registry/
design-spec/gallery-coverage/state-matrix/overlay-registry. Both exited 0.)

## Unit tests — `node --import ./scripts/node-test-loader.mjs --test "src/**/*.test.ts"` → 13 pass / 0 fail

- **TEST-1**: PASS  — action callable hook-free outside render, mutates state (real createStoreProxy loaded via the loader → also proves ITEM-10).
- **TEST-2**: PASS  — `$` returns getState() snapshot hook-free outside render.
- **TEST-3**: PASS  — reactive state value read outside render throws (render-only contract).
- **TEST-4**: PASS  — `.__state` is no longer a hook-free special (throws outside render); `$` works, `__setState` unaffected. (Assertion group also covers TEST-10.)
- **TEST-5**: PASS  — `defineStore({actions})` action is function-typed on getState and callable hook-free through the proxy.
- **TEST-6**: PASS  — `defineLocalStore().use()` (react-dom/server render): reactive read in render + `$` snapshot + handler-context action call + `__state` not a snapshot alias.
- **TEST-9**: PASS  — `$.__destroy__()` reachable hook-free and does NOT trigger store `__init__` (HMR-destroy pattern).
- **TEST-10**: PASS  — shared proxy exposes only `$` (no `__state`) as a hook-free special; desktop-consumed contract (verified in the TEST-4 assertion group; desktop sweep additionally tsc-backstopped by `npm run check (desktop/ui)`).
- **TEST-11**: PASS  — the `no-store-internal-state.grit` guardrail flags a `.__state` member-access fixture in BOTH workspaces (`biome lint --only=style/noRestrictedImports` → "was removed" diagnostic), and the clean tree passes `lint:guardrails` inside `npm run check`.
- **TEST-12**: PASS  — after `gen:state-matrix`, `check:state-matrix` reports "state matrix up to date" in BOTH workspaces (no drift; regenerated matrix matches swept source).

## E2E — Playwright `--workers=1` (full stack: cargo-run backend + Vite preview + Postgres)

- **TEST-7**: PASS  — `tests/e2e/chat/conversation-list-search.spec.ts` → 2 passed (1.7m). Typing in the conversation-list search drives the swept `Stores.ChatHistory.setSearchQuery`/`loadConversations` handlers (formerly `.__state.*`) — direct action calls from a real onChange handler work end-to-end.
- **TEST-8**: PASS  — `tests/e2e/projects/conversation-list-interaction.spec.ts` → 1 passed (27.4s). Conversation delete drives the swept `Stores.ChatHistory.deleteConversation` handler from a real onClick.

All enumerated TEST-IDs PASS; both touched frontend workspaces have a passing `npm run check`.
