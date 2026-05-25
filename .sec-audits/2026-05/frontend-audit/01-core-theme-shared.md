# Core, theme, shared infrastructure audit

## Summary
- 1 HIGH, 9 MED, 9 LOW findings across the core meta-framework, themes, ThemeProvider, hooks, utils and api-client.
- **Revision note (2026-05-23 post-review):** the original audit listed 4 HIGH findings under `createStoreProxy` (B-1, B-2, B-3, B-4) framed as "rules-of-hooks violation". On re-review the proxy design is **sound by convention** — see §B-1 below for the corrected analysis. The real concern is documentation (the implicit "state-value path is component-only" contract is not documented). Per-store `__destroy__` cleanup gaps that B-3 conflated with the proxy live in `09-cross-cutting-correctness.md` B-9 and remain HIGH there.
- `src/core/permissions/` (Auth/Permissions plan) is NOT yet present in the tree — **plan not yet applied**; no findings from that subtree.
- No top-level React error boundary anywhere — a single throw inside any registered component or `initialize()` crashes the entire shell white-screen (B-12, the remaining HIGH).
- Theme parity drift: `colorBgMask`, `colorTextSecondary`, `colorBgContainer`, several `Tag`/`Menu`/`Button` fixes exist in `light.ts` but are missing or only partly mirrored in `dark.ts`; `override.ts` re-applies tokens that `light.ts` already sets, which can mask intentional theme switches.

---

## Bugs

### B-1 — `createStoreProxy` design: the "state-value path is component-only" contract is undocumented  [LOW]  *(revised 2026-05-23)*
**File:** `src/core/stores.ts:173-244`
**What:** **REVISED.** The original audit framed this as a "rules-of-hooks violation". On re-review, the proxy's `Proxy.get` is deliberately split into three branches:
1. **Special properties** (`__state`, `__setState`, `__refCount`, `__refTracker`, `__destroyed`) at lines 175-190 — return directly, no hooks.
2. **Function values (actions)** at lines 213-215 — return directly, no hooks. Callable from event handlers, store actions, non-component code.
3. **Nested store proxies** (objects with `__refTracker`) at lines 222-224 — return directly, no hooks. The inline comment at lines 218-221 explicitly documents this: *"This allows accessing nested stores from event handlers without hook errors."*
4. **Plain state values** at lines 226-241 — calls `useEffect` + `useStore(useShallow(...))`. **Implicit contract: this branch must only be entered during a component render.**

Whether a given `(store, prop)` enters branch 4 is determined by what `state[prop]` IS, and that's **stable across renders** — actions remain functions, state values remain state values for the lifetime of the store. So per-component hook order is preserved, equivalent to any normal custom hook. The pattern is sound by convention: *"`Stores.X.action()` is safe anywhere; `Stores.X.field` only inside components."* That's the same contract React already enforces for any custom hook.

The real concern is documentation, not correctness: the implicit "branch 4 is component-only" contract lives only in the comment at lines 218-221 (which is scoped to the nested-store path). A new contributor who reads `const u = Stores.Auth.user` inside a non-component callback will get an "Invalid hook call" at runtime with no obvious explanation. The pattern self-documents through the runtime error (you can't silently break things), but the dev-experience cost is real.

**Fix sketch:** Add a JSDoc header to `createStoreProxy` (and to the `Stores` proxy at line 273) documenting the four branches and the component-only contract. Cross-reference from `.claude/REACT_COMPONENT_PATTERNS.md`. Optionally add an ESLint rule against reading non-special, non-action store properties from non-component contexts — implementable as a custom rule that flags `Stores.X.field` outside React render functions.

### B-2 — *(removed)*  *(revised 2026-05-23)*
Originally framed as a HIGH bug ("StrictMode double-count schedules destruction"). On re-review: StrictMode's mount → cleanup → mount cycle is synchronous within a microtask in React 18 dev. The `setTimeout` in `scheduleDestroy` has a 5000ms delay; `cancelDestroy` fires within microseconds during the second mount. **No actual destruction occurs.** The only effect is dev-only `console.log` output ("Cancelling destruction…" and "Scheduling store destruction…") from the `import.meta.env.DEV` guards. That's log spam, not a correctness bug, and is stripped in production builds. Removed from the audit.

### B-3 — *(removed; real concern lives in `09-cross-cutting-correctness.md` B-9)*  *(revised 2026-05-23)*
Originally framed as a HIGH bug ("executeDestroy clears `propInitCheck` while observers may still hold subscriptions"). On re-review: `executeDestroy` only runs when `totalCount === 0`, which by definition means every component's `useEffect` cleanup has fired — no observer exists. The 5s grace is precisely to handle "user navigates away and comes back". The duplicate-subscription concern (e.g. Auth re-registering `'onboarding.user_updated'` after re-init) is a **per-store `__destroy__` contract issue**, not a proxy bug — `__destroy__` must unsubscribe its event listeners. That's captured in `09-cross-cutting-correctness.md` B-9 with the actual list of stores that violate the contract (Auth + 3 hub stores).

### B-4 — *(removed; was incorrect)*  *(revised 2026-05-23)*
Originally framed as a MED bug ("`addRef` reset path leaves refcount off-by-one"). On re-review: the "off-by-one" scenario doesn't actually corrupt anything — `removeRef` at lines 83-94 explicitly guards against decrementing from 0 (`if (current > 0) ...`), so a stale `removeRef` after a `reset()` is a silent no-op. That's the safe outcome by design. Removed from the audit.

### B-5 — `removeGroupListeners` and `off` under `immer` produce broken Map mutations  [MED]
**File:** `src/core/events/store.ts:46-128`
**What:** The event bus uses `immer` middleware (`store.ts:42`) but mutates `Map` and `Set` instances inline (`state.handlers.set(...)`, `eventHandlers.delete(...)`). `immer` supports Maps/Sets only if you enable `enableMapSet()` from `immer` — there is no such call anywhere in the codebase. Without that opt-in, `immer` will silently treat them as plain objects and the `produce`-time freezing makes further mutations no-ops in development (when `Object.freeze` is enabled by immer's auto-freeze). In production builds with `immer`'s auto-freeze on, mutating `Map.set` on a frozen Map throws. Either symptom = listeners never get registered properly under some Zustand version paths.
**Fix sketch:** Add `import { enableMapSet } from 'immer'; enableMapSet()` at app bootstrap (e.g. top of `core/events/store.ts`), or refactor to immutable replacement (`state.handlers = new Map(state.handlers); state.handlers.set(...)`).

### B-6 — Event handler errors are caught synchronously but async errors swallowed silently  [MED]
**File:** `src/core/events/store.ts:97-107`
**What:** The handler executor wraps each handler in `try { return Promise.resolve(handler(fullEvent)) } catch (error) { console.error(...); return Promise.resolve() }`. The `try/catch` only catches *synchronous* throws. If a handler returns a rejected promise (the common case for `async` handlers), `Promise.resolve(rejectedPromise)` propagates the rejection. The subsequent `Promise.all(promises)` then rejects, and **`emit()` is itself an async function** — its rejection is bubbled up to the caller of `emit(...)`. Most call sites do `await emitFooCreated(...)` without try/catch, so a single buggy listener can break unrelated mutations.
**Fix sketch:** Wrap each handler call in `Promise.resolve().then(() => handler(fullEvent)).catch(err => console.error(...))` so all rejections are isolated and logged. The current `try/catch` only covers the synchronous throw path.

### B-7 — `removeGroupListeners` doesn't fully purge stale handler entries after the Map iteration ends  [LOW]
**File:** `src/core/events/store.ts:131-162`
**What:** The function iterates `state.groupHandlers.forEach(...)` while collecting keys to delete, then deletes after. That's correct. But the same handler reference could legitimately be registered under two group keys (rare but possible) — `forEach` collects both, then `state.handlers.get(eventType).delete(handler)` runs once per collected key; the second `delete` is a no-op. No correctness bug, but the `console.debug` count at `:158` then reports a misleading "removed N" number.
**Fix sketch:** Dedup by handler before the cleanup loop, or just don't surface the count.

### B-8 — `LazyComponentRenderer`'s lazy detection heuristic misclassifies anonymous arrow components  [MED]
**File:** `src/core/components/LazyComponentRenderer.tsx:68-74`
**What:** The detection rule is *"function with 0 params, no `isReactComponent`, no `.name`"*. Anonymous arrow components (e.g. `const C = () => <div/>` then `export default C`) become `function` with 0 params, no class prototype, and Vite/esbuild typically minifies/strips the name in production. They will be incorrectly classified as lazy imports, wrapped in `React.lazy(fn)`, and the first render will throw "lazy: not a Promise" because `fn()` returns a ReactElement, not a `Promise<{ default }>`. The heuristic is fragile and minification-dependent.
**Fix sketch:** Replace the heuristic with an explicit tag — e.g. `lazyWithPreload` returns a function with a `.__lazy = true` marker that `LazyComponentRenderer` checks. Or use `React.lazy` upstream so the renderer can identify it via `$$typeof === REACT_LAZY_TYPE`.

### B-9 — `LazyComponentRenderer` `useMemo` keyed by `props` object identity causes unnecessary re-mounts  [MED]
**File:** `src/core/components/LazyComponentRenderer.tsx:75-90`
**What:** `useMemo(() => ..., [component, props, isLikelyLazy])` — `props` is a plain object that callers typically construct inline (`props={{ id: 1 }}`), so a fresh reference on every parent render invalidates the memo and **re-mounts** the lazy-wrapped child each tick. For lazy components this is benign-ish (the promise is cached), but it loses internal state on every parent render. For `LazyComponent` wrapped in `<Suspense>` it briefly shows the fallback.
**Fix sketch:** Drop `props` from the deps (the JSX `<Component {...props} />` re-evaluates fine without re-creating the element) or use a stable hash of `props` keys.

### B-10 — `ThemeProvider` `matchMedia` effect cleanup re-runs on every `selectedTheme` change, never cleans up if matchMedia throws  [LOW]
**File:** `src/components/ThemeProvider/ThemeProvider.tsx:25-31`
**What:** The effect depends on `[selectedTheme]`, so each preference toggle tears down and re-creates the listener. That's wasteful but harmless. More importantly, `update()` from `react-use` causes the entire `ThemeProvider` tree to re-render whenever the system preference flips, but `resolveSystemTheme()` is called **inline during render** at `:19`, so it's already correct — the only thing the listener actually does is force a re-render. Equivalent to `useSyncExternalStore`. Fine, but the listener is registered for `selectedTheme !== 'system'` too, which means manual light/dark users still pay for matchMedia notifications they ignore.
**Fix sketch:** Only register the listener when `selectedTheme === 'system'`. Better: replace with `useSyncExternalStore` to get correct hydration behavior.

### B-11 — `ThemeProvider` mutates `document.head` `<meta name="theme-color">` without unique attr → conflicts with HTML-side meta  [LOW]
**File:** `src/components/ThemeProvider/ThemeProvider.tsx:36-47`
**What:** The effect looks up `meta[name="theme-color"]`, creates it if missing, and sets `content` to `currentTheme.token?.colorBgContainer!`. If `index.html` already declares `<meta name="theme-color">` (it often does for PWAs), this clobbers it. The `!` non-null assertion at `:46` silently casts undefined to string in TS, which would set `content="undefined"` if the token is missing.
**Fix sketch:** Add a `data-managed-by="theme-provider"` attribute and only update meta matching that selector; assert `currentTheme.token?.colorBgContainer` and bail if missing.

### B-12 — No top-level error boundary; any single render throw white-screens the entire app  [HIGH]
**File:** `src/App.tsx:50-75`, `src/main.tsx:1-11`
**What:** `App.tsx` maps over `sortedComponents`, but neither it nor `main.tsx` wraps the tree in an error boundary. A single thrown render in any registered module component takes down the whole shell. `LazyComponentRenderer` uses `Suspense` for fallback but has no `<ErrorBoundary>`. This is especially bad for a modular plugin architecture where module crashes should be isolated.
**Fix sketch:** Add a top-level `ErrorBoundary` in `App.tsx` (or `main.tsx`) plus a per-`ConditionalComponent` boundary in `App.tsx` so a single faulty module doesn't crash the whole UI.

### B-13 — Module `initialize()` throwing async errors only `.catch`-logs; sync throws halt subsequent module init  [MED]
**File:** `src/core/module-system/store.ts:162-182`
**What:** `for (const module of modules) { try { module.initialize() } catch ... }` — the sync `try/catch` is present (`:175-180`), so a sync throw in module A is caught and module B still initializes. But the async path `result.catch(...)` only logs; if the promise was supposed to "register slots later" (which `initializeModules` does in step 2 after init), the slot registry runs with partial state and other modules silently miss their slot contributions. There's no `await` and no ordering — Step 2 fires unconditionally right after the sync init loop.
**Fix sketch:** Either make `initializeModules` `async` and `await` each `initialize()` promise sequentially (or in parallel via `Promise.allSettled`), then run slot registration; or document that `initialize()` must be sync (and remove the `Promise` branch entirely).

### B-14 — `registerModule`'s `onModuleRegister` is invoked twice for the new module from `loader.ts`  [MED]
**File:** `src/modules/loader.ts:113-135`, `src/core/module-system/store.ts:129-139`
**What:** Inside `registerModule` (store), after pushing the new module the store calls `state.modules.forEach(existingModule => existingModule.onModuleRegister?.(module))` and also calls the new module's hook for existing modules (`:135-139`). Then `loader.ts:117-135` does **the same thing again** after `registerModule` returns. Net effect: every existing module's `onModuleRegister` is fired twice per registration (once from store, once from loader), and the new module's hook is also fired twice on each existing module. Modules that use this hook for one-shot registration of routes/widgets will register them twice → duplicate routes, duplicate slot items, duplicate event listeners (depending on `groupName` dedup).
**Fix sketch:** Pick one location — recommend removing the duplicate `forEach` in `loader.ts:117-135` since `registerModule` already handles it.

### B-15 — `removeRef` recurses indirectly via `cancelDestroy` cleanup path inside `addRef` and can deadlock under StrictMode if `__destroy__` is async  [LOW]
**File:** `src/core/stores.ts:115-156`
**What:** `executeDestroy` calls `state.__destroy__()` which may return a promise; the `result.catch(...)` is the only handling. Meanwhile the tracker has already flipped `destroyed = true` and cleared init flags. If a new component mounts during the in-flight async destroy, `addRef` sees `destroyTimeoutId === null && destroyed === true` and calls `reset()` — *while the previous `__destroy__()` is still running*. The destroy promise's later resolution does nothing meaningful (only logs), but if `__destroy__` was supposed to clean event listeners, those listeners now get re-registered by `__init__.__store__` from the fresh `reset()`, then a moment later `__destroy__` continues and may double-unsubscribe.
**Fix sketch:** Track the in-flight destroy promise on the tracker; `addRef` should await it (or reset only after it settles) before re-initialising.

### B-16 — SSE reader leaked when the SSE branch's `while (true)` loop exits via abort but `done` was never reached  [LOW]
**File:** `src/api-client/core.ts:330-374`
**What:** The abort path at `:335-338` releases the lock and breaks, which is correct. But on `done = true` exit (normal completion at `:341`), the loop simply breaks without releasing the lock or calling `reader.cancel()`. In most cases the response is already exhausted so the lock is released by the runtime — but for partial servers that send a final empty record then close, the lock can linger until GC.
**Fix sketch:** `try { ... } finally { try { reader.releaseLock() } catch {} }` to make cleanup unconditional.

### B-17 — `getAuthToken` blindly `JSON.parse`s `localStorage['auth-storage']`; corrupt JSON throws → unhandled  [MED]
**File:** `src/api-client/core.ts:10-18`
**What:** `JSON.parse(authData)` is wrapped in no try/catch. If the auth storage gets corrupted (browser eviction half-write, dev migrations, manual user edit), every subsequent API call throws synchronously from `getAuthToken()`, which is called from `callAsync` *before* any per-request error handling. The error surfaces as a generic unhandled exception, not a 401.
**Fix sketch:** Wrap parse in try/catch, return `null` on failure, and consider logging once + clearing the bad key.

### B-18 — `callAsync` URL parameter interpolation skips `encodeURIComponent` on path params  [MED]
**File:** `src/api-client/core.ts:99-108`
**What:** Path captures are substituted via `endpointPath.replace(\`{${capture}}\`, params[c])` with no encoding. Any param containing `/`, `?`, `#`, or spaces will break the URL. Even worse: a malicious or careless caller can inject path traversal (e.g. `..%2F..`). The GET query branch encodes correctly (`:117-119`), so the inconsistency is the bug.
**Fix sketch:** `endpointPath.replace(\`{${capture}}\`, encodeURIComponent(String(params[c])))` for both the JSON and FormData branches.

---

## Inconsistencies

### I-1 — Light theme defines tokens that dark theme omits (theme parity drift)  [MED]
**File:** `src/themes/light.ts:13-114` vs `src/themes/dark.ts:11-103`
**What:** Tokens present in light but missing/under-specified in dark:
- `colorBgMask` (`light.ts:19`) — no `colorBgMask` in dark.ts. Modals/Drawers will inherit the antd default mask, which on a `#1e1e1e` background may not have intentional contrast.
- `colorTextSecondary` (`light.ts:25`) — no equivalent override in dark.ts. The "Descriptions labels" fix exists for both, but other `colorTextSecondary` consumers (e.g. `Form` extra text) get default opacity-45 in dark mode, which on `#1e1e1e` is below WCAG.
- `Button.colorErrorHover` — light uses `#ff4d4f` (`light.ts:48`), dark uses `#ff4d4f` too (`dark.ts:43`) — same hue, but with a dark background the "hover" being lighter than the rest doesn't read as a state change.
- `Tag.colorErrorBg` differs in saturation between modes — dark `#321414`, light `#ffccc7` — fine; but `Tag.colorErrorText` is `#a8071a` (light) vs `#ff7875` (dark) — only dark mirrors the global `colorError` token.
- `Card` overrides (`light.ts:66-69`) — also defined in `override.ts` (re-applied to dark via `...ComponentOverrides` spread) but light defines them inline, so any override in `override.ts` is silently masked by light's redefinition (`light.ts:66-69`).
**Fix sketch:** Build a "token parity" snapshot test that imports both themes and asserts the same keyset under `token`/`components`. Move common overrides into `override.ts` and consume them in both `light.ts` and `dark.ts` rather than redefining.

### I-2 — `override.ts` re-applies tokens that `light.ts` also sets, masking theme switches  [LOW]
**File:** `src/themes/override.ts:1-20`, `src/themes/light.ts:9-12`
**What:** `TokenOverrides` defines `fontSize: 16, fontSizeIcon: 16, borderRadius: 6, padding: 6`. `light.ts` also sets the same four tokens inline (`:9-12`). `dark.ts` spreads `TokenOverrides` (`:12`). So any future change to `override.ts` only affects dark mode, breaking the assumption that "override.ts" is the single source of truth.
**Fix sketch:** Either have `light.ts` spread `TokenOverrides` too, or move the common values into a shared module that both themes import.

### I-3 — `EventBus` group dedup separator collision with event-type colons  [LOW]
**File:** `src/core/events/store.ts:50-63, 131-141`
**What:** Group keys are built as `\`${groupName}::${eventType}\``. The split on `:158-159` uses `key.substring(prefix.length)` where `prefix = \`${groupName}::\``. If a `groupName` contains `::`, dedup misbehaves but consumers control that. The deeper issue: there's no protection against `groupName` colliding with an event type. Low risk in practice.
**Fix sketch:** Document the constraint or use a `Map<string, Map<string, handler>>` (group → event → handler) instead of a flat string-key map.

### I-4 — `ThemeProvider` uses `react-use`'s `useUpdate` for a job that `useSyncExternalStore` would handle natively  [LOW]
**File:** `src/components/ThemeProvider/ThemeProvider.tsx:23-31`
**What:** `useUpdate` is a force-rerender hammer. The native React 18 idiom for subscribing to media-query changes is `useSyncExternalStore`. Drop a dependency and get hydration-correctness for free.
**Fix sketch:** Replace with `useSyncExternalStore((cb) => { mq.addEventListener('change', cb); return () => mq.removeEventListener('change', cb) }, () => mq.matches ? 'dark' : 'light')`.

### I-5 — Light/dark `Modal` overrides asymmetric  [MED]
**File:** `src/themes/dark.ts:46-50`, `src/themes/light.ts` (absent)
**What:** Dark theme sets `Modal.contentBg`/`footerBg`/`headerBg = BaseBackgroundColor`. Light theme has no Modal override at all. The defaults differ, which is fine, but the asymmetric customization makes the modal "feel" different across modes — e.g. light modal header has antd's subtle gradient, dark modal header is flat.
**Fix sketch:** Either remove the dark override (let antd defaults render) or mirror in light.

### I-6 — Module hooks `onModuleRegister` fire ordering depends on registration order, not topological dependency  [MED]
**File:** `src/modules/loader.ts:114-135`, `src/core/module-system/store.ts:129-139`
**What:** The loader does `for (const module of sortedModules) registerModule(module)` so modules register in dependency order. But `onModuleRegister` is called *during* registration on already-registered modules — so a module's hook receives modules in dependency order. That's correct for cross-module discovery. **However**: combined with the double-fire bug (B-14) the order observers see is `existingModules.forEach(o => o.hook(new))` then `new.hook(o)` for each old, then the loader does the SAME loop. So observers see each pairing twice in different orders.
**Fix sketch:** Fix B-14 first; verify ordering with a small unit test.

### I-7 — `LazyComponentRenderer` default fallback uses `p-3 flex justify-center` but elsewhere modules use `Spin tip` patterns  [LOW]
**File:** `src/core/components/LazyComponentRenderer.tsx:61-64`
**What:** The library-default fallback is a small spinner with padding. Some modules pass `fallback={null}` (App.tsx:32), some use a custom spinner. Inconsistency between full-page vs widget contexts.
**Fix sketch:** Provide two named exports: `defaultLazyFallback` and `inlineLazyFallback`, document when to use each.

### I-8 — `RegisteredStores` interface is declared in `core/stores.ts` and augmented by every module via `declare module '@/core/stores'`, but the file is also `export *` from `@/core` — TS resolution depends on import path  [LOW]
**File:** `src/core/stores.ts:262-269`, multiple `*.store.ts` files declaration-merging
**What:** Modules augment via `declare module '@/core/stores'`. If anyone augments via `declare module '@/core'` (because of `export * from './stores'` in `core/index.ts`), the augmentation silently misses. Low practical risk — convention is followed — but enforce with a lint rule.
**Fix sketch:** Add a custom eslint rule or a comment in `core/index.ts` forbidding declaration merging through that re-export.

---

## Inefficiencies

### E-1 — Every property access on `Stores.X.foo` recreates a `useShallow` selector inline  [MED]
**File:** `src/core/stores.ts:239-241`
**What:** `useStore(useShallow((state) => state[prop]))` constructs a fresh selector function on every render. `useShallow` is intended to wrap a stable selector. As written, every render gets a new selector reference, which (depending on `useShallow`'s memoization) can cause subscribed-state recomputation even when nothing changed.
**Fix sketch:** Either (a) lift selectors per-prop into a `Map<string|symbol, () => any>` cached on the proxy, or (b) drop `useShallow` for primitive props (it's only needed for objects).

### E-2 — `usePrefetchModules` walks every route on every `routes` change and re-runs every preload promise  [MED]
**File:** `src/hooks/usePrefetchModules.ts:14-37`
**What:** Each call to a `lazyWithPreload(...)` factory is cached — calling the function multiple times returns the same promise, so the actual fetch is one-shot. But the `forEach` walk runs every time `routes` updates. If modules add routes incrementally (as they do during boot), this fires N times for N route registrations.
**Fix sketch:** Track which routes have been preloaded in a `WeakSet` and skip them on subsequent runs.

### E-3 — `accessibilityFixes` MutationObserver observes the entire document body with `subtree: true, childList: true`  [MED]
**File:** `src/utils/accessibilityFixes.ts:98-104`
**What:** Observing every DOM mutation on `document.body` is heavy. Every keystroke in a complex form triggers MutationRecords (antd virtualises lists, animations, ripple effects). Plus, `removeAriaRequiredFromSelects` runs `document.querySelectorAll('.ant-select[aria-required="true"]')` on every childList mutation — an O(N) DOM scan per mutation. On busy pages with many selects, this is measurable.
**Fix sketch:** Scope observer to `attributes: true, attributeFilter: ['aria-required']` only (drop `childList: true, subtree: true`), or debounce the childList branch to next tick.

### E-4 — Theme `tinycolor(BaseBackgroundColor).darken(...)` computed at module import time but values are static — OK, but repeated  [LOW]
**File:** `src/themes/light.ts:16-19`
**What:** Five tinycolor computations on module load. Not a perf issue, just noisy.
**Fix sketch:** Replace with pre-computed hex strings or cache the result.

### E-5 — `App.tsx` re-sorts components on every render via `useMemo([components])`, but `components` array identity changes whenever a new component is added by HMR  [LOW]
**File:** `src/App.tsx:63-65`
**What:** Sort is cheap; not a real problem. Just noting that in dev the sort runs frequently.
**Fix sketch:** N/A — keep as is.

### E-6 — Decoder buffer never flushed on stream end → trailing partial line lost  [LOW]
**File:** `src/api-client/core.ts:327-345`
**What:** On `done: true`, the `buffer` may still contain a partial line if the server didn't send a trailing `\n`. The loop exits without processing it. Most SSE servers do send the trailing newline, but spec-conformant clients should flush.
**Fix sketch:** After the `while`, do `if (buffer) { /* parse remaining as one line */ }`.

---

## Responsive / sizing / scrolling

### R-1 — `index.css` `.ant-app { height: 100dvh; width: 100dvw }` is the only viewport-sizing rule and can clip if antd wraps content in fragments  [MED]
**File:** `src/index.css:28-31`
**What:** Agent 2 covers the layout interaction; my note here: `.ant-app` is a `<div>` rendered by antd's `<App>` component. The CSS sets it to dynamic viewport (`dvh`/`dvw`) which is the *correct* behavior on iOS Safari (avoids the URL bar bounce). However, `#root` is `height: 100%` (`:21-23`) but its parent `<body>` has no explicit height. On most browsers `body` defaults to `auto`, so `100%` resolves against an undefined containing block — in practice browsers treat `html` as the viewport and `body` shrinks to content. Result: `#root` may be 0px tall before `<ThemeProvider>` mounts (briefly during first paint), and you see a flash of empty viewport.
**Fix sketch:** Add `html, body { height: 100%; }` in `@layer base` so `#root: 100%` resolves consistently.

### R-2 — `DivScrollY` adds `overflow-y-auto` AND wraps content in OverlayScrollbars → double scroll on small heights  [MED]
**File:** `src/components/common/DivScrollY.tsx:22-32`
**What:** The component renders `<OverlayScrollbarsComponent className="overflow-y-auto flex ...">` — `OverlayScrollbarsComponent` provides its own scroll. The Tailwind `overflow-y-auto` on the root forces the host element to scroll too. On viewports where content overflows by just a few pixels, you get a native scrollbar AND the overlay scrollbar competing for events. On touch devices the native one usually wins, defeating the point of OverlayScrollbars.
**Fix sketch:** Drop `overflow-y-auto` from the merged class (the OverlayScrollbars wrapper handles it). Or set `overflow: hidden` on the host and let OS handle internally.

### R-3 — `DivScrollY` inner `<div className="flex flex-col">` swallows any `display` prop set by callers  [LOW]
**File:** `src/components/common/DivScrollY.tsx:33`
**What:** Hardcoded `flex flex-col` for the inner container. Callers that want a non-flex layout (e.g. a grid for cards inside a scrollable region) cannot override it because the inner div doesn't receive `restProps`.
**Fix sketch:** Accept an `innerClassName` or `innerStyle` prop.

### R-4 — `prefers-color-scheme` listener fires `update()` which causes full subtree re-render → expensive on themed pages  [LOW]
**File:** `src/components/ThemeProvider/ThemeProvider.tsx:25-31`
**What:** Each OS theme toggle re-renders the entire tree under `<ThemeProvider>` because antd's `ConfigProvider` propagates the theme as a context. On low-end devices this can stutter. Not specifically a responsive concern, but it's the closest thing to one in scope.
**Fix sketch:** Same as I-4 — use `useSyncExternalStore`.

### R-5 — No `min-height` floor on any of the meta-framework component wrappers; lazy fallback `Spin` may render at 0px height for some parents  [LOW]
**File:** `src/core/components/LazyComponentRenderer.tsx:61-64`
**What:** `<div className="p-3 flex justify-center"><Spin size="small" /></div>` — if the parent is a flex container with `align-items: stretch` AND height 0, the spinner becomes invisible. Most callers pass `fallback={null}` (App.tsx:32) so this rarely surfaces.
**Fix sketch:** Add `min-h-[2rem]` to the default fallback.

---

## Permissions plan (note)

**Plan not yet applied.** `src/core/permissions/` does not exist in this tree (verified — no such directory). When the plan lands, audit specifically:
- `evaluatePermission`'s `is_admin` short-circuit: must run BEFORE wildcard resolution, otherwise wildcards in the permission database can override deny-by-default for non-admin users.
- `*` vs `resource::*` precedence: ensure `*` (global wildcard) trumps `resource::*` and both trump exact match. Verify the separator is `::` consistently (used by the event-bus group dedup; mixing `::` and `:` would silently break).
- `usePermission` React hook integration: must read `Stores.Auth.user` and `Stores.Auth.permissions` *reactively* (i.e., subscribed via the proxy). If implemented as a one-shot read of `Stores.Auth.__state.user`, it won't re-render on login/logout — and given B-1/B-2 above, getting the reactive path right is non-trivial.
- `<Can>` component: must render `null` (not a fallback) when permission denied, to avoid layout shift. Document the empty-render contract.
