# PLAN — store-kit-hookfree-actions

## Context / discovered ground truth

The reactive store proxy already lives in ONE shared file,
`src-app/ui/src/core/stores.ts` (`createStoreProxy`) + `src-app/ui/src/core/store-kit.ts`
(`createLocalProxy`). The desktop workspace has **no** copy — it resolves
`@/core/stores` / `@/core/store-kit` into `../../ui/src` via the desktop
`localOverridePlugin` + tsconfig `@ziee/ui-core` paths. So there is exactly ONE
proxy implementation to change; both workspaces consume it (desktop adds its own
`declare module '@/core/stores'` augmentations that use the shared `StoreProxy<T>`).

The get-trap **already** returns function-valued props (actions) resolved
directly from `useStore.getState()`, hook-free (stores.ts:254-257,
store-kit.ts:277). So user-goal (1) — "actions callable directly anywhere" — is
already the runtime behavior; this feature makes that guarantee **explicit +
tested + lint-locked**, and removes the now-redundant `__state` alias (goal 2).

`$` and `__state` are currently the SAME branch (`prop === '$' || prop === '__state'`,
both return `useStore.getState()`), so removing `__state` and pointing every
snapshot read at `$` is behavior-preserving.

The guardrail mechanism is a Biome **GritQL plugin** (proven: a `` `$obj.__state` ``
member-expression pattern fires, and plugins DO run under
`lint:guardrails` = `biome lint --only=style/noRestrictedImports src`). Desktop's
`biome.json` references the shared `../../ui/biome-plugins/*.grit`.

## Items

- **ITEM-1**: Remove the `__state` alias from `createStoreProxy`'s get-trap in `src-app/ui/src/core/stores.ts` — the special-prop branch becomes `if (prop === '$')` only, still returning `useStore.getState()`. `$` is the sole hook-free snapshot escape.
- **ITEM-2**: Drop `__state` from the proxy TYPES in `stores.ts` — remove the `__state` member from both arms of `ExtractZustandState<T>` and from `StoreProxy<T>` — leaving `$`, `__setState`, `__refCount`, `__refTracker`, `__destroyed`. Rewrite the get-trap doc-comment + `StoreProxy` JSDoc so they document `$` (not `__state`) as the handler-side snapshot.
- **ITEM-3**: Remove the `__state` alias from `createLocalProxy` in `src-app/ui/src/core/store-kit.ts` (`if (prop === '$' || prop === '__state')` → `if (prop === '$')`). `LocalStoreInstance<FullState>` already exposes only `$` — no type change needed there; confirm.
- **ITEM-4**: Make the function-detection invariant explicit in BOTH `createStoreProxy` and `createLocalProxy`: an action (function-valued prop) is always returned resolved from `getState()`, hook-free, callable in render AND handlers. No behavior change (branch already present) — clarify the comment; this is the anchored guarantee the action-callable tests lock in.
- **ITEM-5**: Update the shared core consumer `src-app/ui/src/core/module-system/store.ts` HMR-destroy path: `oldStoreProxy?.__state?.__destroy__` / `oldStoreProxy.__state.__destroy__()` → `oldStoreProxy?.$?.__destroy__` / `oldStoreProxy.$.__destroy__()` (`$` returns getState() incl. `__destroy__`, and — unlike bare `.__destroy__` proxy access — skips the trap's init side-effect).
- **ITEM-6**: Codebase-wide `.__state` sweep across `src-app/ui/src` — per site: `Stores.X.__state.method(...)` → `Stores.X.method(...)` (direct action); `Stores.X.__state.field` → `Stores.X.$.field`; whole-object `Stores.X.__state` (destructured / assigned) → `Stores.X.$`; `proxyVar.__state.foo` → `proxyVar.$.foo`; and rewrite every `.__state`-referencing comment/doc to the `$`/direct guidance.
- **ITEM-7**: Same `.__state` sweep across `src-app/desktop/ui/src` (MagicLinkPage.tsx, desktop-base/module.tsx, host-mount ConversationMountsControl.tsx).
- **ITEM-8**: Add a Biome GritQL guardrail `src-app/ui/biome-plugins/no-store-internal-state.grit` banning `$obj.__state` member access, and register it in the `plugins` array of BOTH `src-app/ui/biome.json` and `src-app/desktop/ui/biome.json` (desktop uses the shared `../../ui/biome-plugins/no-store-internal-state.grit` path), alongside the existing `no-raw-interactive-elements.grit`.
- **ITEM-9**: Regenerate the gallery state-matrix in BOTH workspaces via `npm run gen:state-matrix` so `stateMatrix.generated.ts` + `STATE_MATRIX.md` reflect the swept `.$.` source (generated files are never hand-edited), keeping `check:state-matrix` green.
- **ITEM-10**: Add a `node --test` alias-resolver loader (`src-app/ui/scripts/node-test-loader.mjs` + `node-test-hooks.mjs`) plus two boundary stubs (`src-app/ui/src/core/__test-stubs__/{module-system,events}.ts`) so unit specs can import the REAL proxy modules (which use `@/`-aliases + a couple of browser-coupled boundaries the proxy factory never exercises); point `test:unit` at the loader. The proxy logic + React + zustand stay real — only the two unrelated boundaries are stubbed. Enables the ITEM-1..4 unit tests to run against real code under the repo's existing `node --test` runner (no new heavy deps: react-dom/server is already installed).

## Files to touch

- `src-app/ui/src/core/stores.ts` (ITEM-1, ITEM-2, ITEM-4)
- `src-app/ui/src/core/store-kit.ts` (ITEM-3, ITEM-4)
- `src-app/ui/src/core/module-system/store.ts` (ITEM-5)
- `src-app/ui/src/**` app/module files listed in the sweep (ITEM-6) — workflow, projects, chat, chat-extensions (text/export), mcp chat-extension + components, file (viewers/chrome, chat-extension), assistant picker, summarization, hub, llm-provider DownloadIndicatorWidget, skill, user, onboarding FinishStep, app/module.tsx, layouts app-layout
- `src-app/desktop/ui/src/modules/tunnel-auth/MagicLinkPage.tsx`, `src-app/desktop/ui/src/modules/desktop-base/module.tsx`, `src-app/desktop/ui/src/modules/host-mount/conversation-extension/components/ConversationMountsControl.tsx` (ITEM-7)
- `src-app/ui/biome-plugins/no-store-internal-state.grit` (new), `src-app/ui/biome.json`, `src-app/desktop/ui/biome.json` (ITEM-8)
- `src-app/ui/src/dev/gallery/{stateMatrix.generated.ts,STATE_MATRIX.md}` + `src-app/desktop/ui/src/dev/gallery/{stateMatrix.generated.ts,STATE_MATRIX.md}` (ITEM-9, generated — via gen script only)
- New unit tests: `src-app/ui/src/core/stores.test.ts`, `src-app/ui/src/core/store-kit.test.ts` (Phase 3)
- Test infra (ITEM-10): `src-app/ui/scripts/node-test-loader.mjs`, `src-app/ui/scripts/node-test-hooks.mjs`, `src-app/ui/src/core/__test-stubs__/module-system.ts`, `src-app/ui/src/core/__test-stubs__/events.ts`, and the `test:unit` script in `src-app/ui/package.json`

## Patterns to follow

- **Proxy edit** — mirror the EXISTING special-prop branches + function-branch idiom already in `createStoreProxy` / `createLocalProxy`; the change is a deletion of the `__state` alias, not a new shape.
- **Guardrail plugin** — mirror `src-app/ui/biome-plugins/no-raw-interactive-elements.grit` exactly (GritQL `language js` + `register_diagnostic`), registered the same way in `biome.json` `plugins`.
- **Generated state-matrix** — treat like every other `gen:*`/`check:*` pair in the repo: edit source, run the `gen:` script, commit the regenerated artifact; never hand-edit `*.generated.ts`.
- **Unit tests** — mirror the existing store-kit/proxy `#[cfg(test)]`-analog Vitest specs under `src-app/ui/src` (co-located `*.test.ts`); use React Testing Library `renderHook` for the render-vs-handler distinction (same tooling the UI suite already uses).
