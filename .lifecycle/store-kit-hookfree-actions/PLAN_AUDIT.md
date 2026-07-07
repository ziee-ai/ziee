# PLAN_AUDIT — store-kit-hookfree-actions

Audited the plan against the actual codebase (worktree off origin/main @ 786b2689).

## Breakage risk

- **Type removal is the safety net, not a risk.** Removing `__state` from
  `ExtractZustandState<T>` + `StoreProxy<T>` makes ANY remaining `.__state`
  access a `tsc` error in BOTH workspaces. A full-repo grep confirms all **85**
  `.__state` occurrences (excluding the distinct `__setState`) live under
  `src-app/ui/src` + `src-app/desktop/ui/src` — **zero** in `tests/`, `scripts/`,
  gallery detector fixtures, or `src-app/server`. So the sweep set is closed; the
  two-workspace `tsc` (via `npm run check`) is a complete backstop for a missed
  site.
- **`$` ≡ `__state` today** — both are the same get-trap branch returning
  `useStore.getState()`. So repointing every snapshot read from `.__state` to
  `.$` is byte-for-byte behavior-preserving; no consumer observes a different
  value. (Resolved as DEC-1.)
- **Actions already hook-free** — the function-branch (`typeof value ===
  'function' → return value`) predates this work in BOTH `createStoreProxy` and
  `createLocalProxy`. So converting `.__state.action()` → `.action()` cannot
  regress: `.action` was already resolved from `getState()` without a hook. The
  only new guarantee is *lint-locked + tested*, not *new runtime behavior*.
- **Internal HMR path** (`module-system/store.ts`) uses
  `oldStoreProxy.__state.__destroy__()`. Switching to `.$.__destroy__()` is
  strictly safer: the `$` branch returns early (before the trap's init
  side-effect), whereas a bare `.__destroy__` proxy access would trigger
  store-level `__init__` as a side effect of tearing down. (Resolved as DEC-3.)
- **`__setState` untouched** — the grit ban pattern `$obj.__state` matches the
  exact member name `__state` only; `.__setState` (5 sites) is a different
  identifier and is neither swept nor flagged. Verified against the grit AST
  semantics (member-expression name match).
- **Guardrail false-positive risk: none.** Post-sweep, the only string
  `__state` remaining is inside regenerated `stateMatrix.generated.ts` — and it
  becomes the STRING `"…$.isAuthenticated"`, not a `.__state` member expression,
  so the grit `$obj.__state` pattern cannot match it.

## Pattern conformance

- **Proxy edit** conforms — it deletes the `__state` alias from the existing
  special-prop branch; no new proxy shape. Mirrors the current idiom.
- **Guardrail** conforms — a Biome GritQL plugin identical in form to the
  shipped `no-raw-interactive-elements.grit`, registered in the same `plugins`
  array of both `biome.json` files (desktop references the shared `../../ui/…`
  path, exactly as it already does for the raw-interactive plugin). Empirically
  verified: a `$obj.__state` grit pattern fires, and plugins DO run under
  `lint:guardrails` (= `biome lint --only=style/noRestrictedImports src`).
- **Generated state-matrix** conforms — edit source, run the repo's own
  `gen:state-matrix` script, commit the artifact; `check:state-matrix` (inside
  `npm run check`) gates drift. No hand-editing of `*.generated.ts`.
- **Unit tests** conform — co-located `*.test.ts` run by the repo's existing
  `test:unit` (`node --test`), matching `chat/core/tool-status.test.ts`. The
  ITEM-10 loader is the minimal, honest bridge (`@/` alias + two unrelated
  boundary stubs) so the REAL proxy modules import under `node --test`; no new
  test framework, no RTL (react-dom/server is already a dep). Verified: real
  `createStoreProxy` + `defineLocalStore` load and their action/`$`/reactive
  behaviors are observable.

## Migration collisions

N/A — this feature adds **no** database migrations and touches no `migrations/`
files. `ls migrations/` unaffected.

## OpenAPI regen

N/A — no backend types, no OpenAPI schema, no `api-client/types.ts` change. This
is a pure frontend-core + lint + test change. The phase-3/phase-8 frontend gates
apply (UI workspaces touched); the backend gate does not.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — deletes the `__state` alias from `createStoreProxy`'s `$`/`__state` branch; `$` remains and already returns `getState()`. Behavior-preserving per DEC-1.
- **ITEM-2** — verdict: PASS — removes `__state` from `ExtractZustandState`(both arms) + `StoreProxy` and rewrites the JSDoc. tsc-enforced completeness; no consumer keeps a valid `.__state` type.
- **ITEM-3** — verdict: PASS — same one-line alias deletion in `createLocalProxy`; `LocalStoreInstance` already exposes only `$`, so no type edit needed. Verified the type shape.
- **ITEM-4** — verdict: PASS — function-branch already returns actions resolved from `getState()` in both proxies; item is a comment-clarification + the anchored test. No behavior change ⇒ zero regression risk.
- **ITEM-5** — verdict: PASS — `module-system/store.ts` HMR-destroy path switches to `.$.__destroy__()`; strictly safer (skips init side-effect). Internal-only caller.
- **ITEM-6** — verdict: PASS — the `src-app/ui/src` sweep; per-site rule is deterministic (action→direct, field→`$.field`, whole-object→`$`). All sites enumerated from the full grep; tsc is the completeness backstop.
- **ITEM-7** — verdict: PASS — the `src-app/desktop/ui/src` sweep (3 files: MagicLinkPage, desktop-base/module, host-mount ConversationMountsControl); same rule; desktop consumes the shared proxy/type, so its `tsc` (`npm run check`) backstops it.
- **ITEM-8** — verdict: PASS — new grit guardrail + registration in both `biome.json`s; mechanism + firing-under-`lint:guardrails` empirically proven; no false positives (see Breakage risk).
- **ITEM-9** — verdict: PASS — regenerate via `gen:state-matrix` in both workspaces; the generator re-derives the condition strings from swept source; `check:state-matrix` gates it. Conventional gen/check pair.
- **ITEM-10** — verdict: PASS — node-test loader + two boundary stubs + `test:unit` rewire; loader intercepts only `@/` (relative/bare imports pass through, so `tool-status.test.ts` stays green); stubs cover only boundaries the proxy factory never calls. Verified real modules load + tests pass.
