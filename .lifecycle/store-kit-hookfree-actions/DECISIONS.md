# DECISIONS — store-kit-hookfree-actions

### DEC-1: Does removing `__state` change snapshot semantics vs `$` (breaking existing `$.field` reads)?
**Resolution:** No. In the current get-trap both are the SAME branch: `if (prop === '$' || prop === '__state') return useStore.getState()`. `$` keeps returning `useStore.getState()` (the full state snapshot incl. actions + lifecycle) verbatim, so every existing `Stores.X.$.field` read is byte-for-byte unchanged. Only the `__state` alias is deleted.
**Basis:** codebase

### DEC-2: What do call sites that grab the WHOLE `__state` object (`const s = Stores.X.__state`, or destructuring `const { a, b } = Stores.X.__state`) become?
**Resolution:** `Stores.X.$` (i.e. `const s = Stores.X.$` / `const { a, b } = Stores.X.$`). `$` returns `getState()`, so `s.field` reads and `s.action()` calls on that plain snapshot object both keep working (the snapshot is not a proxy — the "no `$` on actions" rule applies to the `Stores.X.$.action()` proxy form, not to a captured snapshot object).
**Basis:** user

### DEC-3: For the internal HMR path in `module-system/store.ts`, use `.$.__destroy__()` or a direct `.__destroy__()`?
**Resolution:** `oldStoreProxy.$.__destroy__()` (and `oldStoreProxy?.$?.__destroy__`). The `$` branch returns early from the trap, so it does NOT trigger the trap's store-level `__init__` side-effect that a bare `.__destroy__` proxy access would fire while tearing a store down.
**Basis:** codebase

### DEC-4: Is the function-detection branch correct for actions authored via `defineStore` / `defineLocalStore`?
**Resolution:** Yes. `makeBuilder` spreads `...actions` into the state object, so `getState()[actionName]` is the action function; the trap's `typeof value === 'function' → return value` returns it directly, hook-free. No change to that branch — this feature only adds tests + a clarifying comment. Verified empirically (real `defineStore`/`defineLocalStore` under `node --test`).
**Basis:** codebase

### DEC-5: Guardrail mechanism — a Biome GritQL plugin or a bespoke node check script?
**Resolution:** A GritQL plugin `src-app/ui/biome-plugins/no-store-internal-state.grit` matching `$obj.__state`, registered in the `plugins` array of both `biome.json`s. It runs inside `npm run check` via `lint:guardrails` (`biome lint --only=style/noRestrictedImports src`) — empirically confirmed that grit plugins fire under `--only`. This mirrors the shipped `no-raw-interactive-elements.grit` exactly.
**Basis:** convention

### DEC-6: Guardrail scope — top-level `plugins` or the scoped override?
**Resolution:** Add it to the SAME override `plugins` array as `no-raw-interactive-elements.grit` (which excludes `components/ui/{kit,shadcn}`, `tests`, and gallery detector fixtures). Excluded paths carry no `.__state` today and are tsc-backstopped anyway (the `__state` type is removed). Matches the established guardrail-scoping convention.
**Basis:** convention

### DEC-7: The per-site sweep rule (action vs field vs whole-object).
**Resolution:** `Stores.X.__state.method(...)` → `Stores.X.method(...)` (direct action, no `$`); `Stores.X.__state.field` → `Stores.X.$.field`; whole-object `Stores.X.__state` (assigned/destructured) → `Stores.X.$`; `proxyVar.__state.foo` → `proxyVar.$.foo` (field) / `proxyVar.method()` (action). Rewrite `.__state`-referencing comments to the `$`/direct guidance.
**Basis:** user

### DEC-8: Generated `stateMatrix.generated.ts` / `STATE_MATRIX.md` — hand-edit or regenerate?
**Resolution:** Regenerate via `npm run gen:state-matrix` in BOTH workspaces after the source sweep; never hand-edit a `*.generated.ts`. `check:state-matrix` (in `npm run check`) gates drift.
**Basis:** convention

### DEC-9: How are the proxy internals unit-tested under `node --test` given `@/` aliases + browser-coupled boundaries?
**Resolution:** A minimal register-hook loader maps `@/…` → real `src/…` (with `.ts/.tsx/index` resolution) and stubs the two boundaries the proxy factory never calls (`@/core/module-system`, `@/core/events`). The local-proxy render test uses `react-dom/server` (already installed) — no RTL, no jsdom, no new deps. Proxy/React/zustand stay real.
**Basis:** codebase

### DEC-10: Modify the repo's `test:unit` script (point it at the loader)?
**Resolution:** Yes — `test:unit` becomes `node --import ./scripts/node-test-loader.mjs --test "src/**/*.test.ts"`. The loader only intercepts `@/` specifiers; relative/bare imports pass through, so the existing `chat/core/tool-status.test.ts` stays green. This keeps ONE unit-test entrypoint and future-proofs `@/`-importing specs.
**Basis:** codebase

### DEC-11: Does desktop need its own proxy edit?
**Resolution:** No. Desktop has no `core/stores.ts` / `core/store-kit.ts`; it resolves `@/core/*` into the shared `../../ui/src` via the `localOverridePlugin` + `@ziee/ui-core` tsconfig paths. One proxy edit serves both. Desktop still gets its own `.__state` sweep (3 consumer files) and its own `npm run check` (tsc) pass.
**Basis:** codebase

### DEC-12: Which proxy "special" props are removed vs kept?
**Resolution:** ONLY `__state` is removed. `$` (its replacement), `__setState`, `__refCount`, `__refTracker`, `__destroyed` are kept — they are internal infra with no clean-alias equivalent and were not named for removal. `__setState` (5 sites) is untouched and is not matched by the `$obj.__state` grit ban.
**Basis:** user
