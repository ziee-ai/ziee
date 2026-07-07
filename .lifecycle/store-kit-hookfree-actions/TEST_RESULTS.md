# TEST_RESULTS — store-kit-hookfree-actions

## Frontend gate (both UI workspaces touched)

npm run check (ui): PASS
npm run check (desktop/ui): PASS

(`npm run check` = tsc + biome guardrails incl. the new `no-store-internal-state`
grit + lint:colors/settings-field/… + check:kit-manifest/testid-registry/
design-spec/gallery-coverage/state-matrix/overlay-registry. Both exited 0.)

> Re-verified AFTER merging origin/main (a1ca389c, F2/F3/model-picker) and
> sweeping the 15 new `.__state` sites that merge introduced. `__state` count in
> source = **0**; ban-lint (`lint:guardrails`) = **0 violations** in both
> workspaces; the grit guardrail still fires on a `.__state` probe.

## Unit tests — `node --import ./scripts/node-test-loader.mjs --test "src/**/*.test.ts"` → 81 pass / 0 fail (13 proxy specs + F2/F3's new specs)

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

- **TEST-7**: PASS  — `tests/e2e/chat/conversation-list-search.spec.ts` → 2 passed (re-run on merged tree; the swept `setSearchQuery`/`loadConversations`/`setSort` handlers on `ConversationList.tsx` drive from real onChange/onClick).
- **TEST-8**: PASS  — `tests/e2e/projects/conversation-list-interaction.spec.ts` → 1 passed (re-run on merged tree; swept `deleteConversation` from a real onClick).

### Post-merge sweep validation (F2/F3 file-viewer specs exercising the newly-swept actions)

- `tests/e2e/file/find-in-document.spec.ts` → PASS — drives swept `Stores.File.setFileFindOpen` (chrome.tsx toolbar + FindableRegion.tsx).
- `tests/e2e/file/image-zoom.spec.ts` → PASS — drives swept `Stores.File.zoomImage` / `setImageViewMode` (image/header.tsx).
- `tests/e2e/file/word-wrap.spec.ts` → PASS — drives swept `Stores.File.setFileWordWrap` (chrome.tsx).

All enumerated TEST-IDs PASS on the post-merge tree; both touched frontend workspaces have a passing `npm run check`; `__state` count = 0; ban-lint 0 violations.
