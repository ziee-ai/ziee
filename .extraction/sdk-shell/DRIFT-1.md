# Chunk `sdk-shell` — DRIFT round 1

Drift = any behavioural divergence between the pre-move ziee shell/permissions
and the extracted packages + app shims, checked symbol-by-symbol against the
pre-move files and confirmed by the live gallery runtime-health run + all 4 tsc
gates.

- **DRIFT-1.1** — verdict: none. Permission evaluator: `hasPermission`
  (is_admin bypass → exact → `*` → hierarchical `a::b::*`) and
  `evaluatePermission` (string leaf, `allOf` AND, `anyOf` OR, null→false) are
  byte-copied; only the `User`/`Permission` type params changed (T-1), which are
  input-position widenings that accept the identical set of runtime values.
- **DRIFT-1.2** — verdict: none. `usePermission` reactive read + `hasPermissionNow`
  snapshot read now route through `authStoreProxy()`, which returns the SAME
  `Stores.Auth` proxy over the SAME `Auth.store`. `Stores.Auth.$` ≡
  `useAuthStore.getState()` (both are the store snapshot) — PROVEN via
  `useAuthStore = Auth.store`. Subscribe/no-subscribe semantics preserved.
- **DRIFT-1.3** — verdict: none. `<Can>` renders `children` on allow / `fallback`
  (default `null`) on deny — verbatim; imports `usePermission`/`PermissionExpr`
  from the sibling framework modules instead of `./`. Fail-closed default intact.
- **DRIFT-1.4** — verdict: none. `ThemeProvider`: the `system`→resolveSystemTheme,
  `.dark`/`.light` class toggle, `applyAccent(root, accentPreset, isDarkMode)`,
  the OS-scheme `matchMedia` re-render, and the one-time `<Toaster/>`+`<DialogHost/>`
  mount are byte-identical; only the config read is a typed seam over the same
  `Stores.ConfigClient`. Accent presets + `applyAccent` CSS-var writes verbatim.
- **DRIFT-1.5** — verdict: none. `useMetaThemeColor` rAF-deferred var read +
  `toRgbHex` 1px-canvas flatten + `setMetaThemeColorFromVar` are verbatim;
  `BlankLayout` useLayoutEffect background set/restore + `main` `display:contents`
  landmark + `useMetaThemeColor('--background')` are verbatim.
- **DRIFT-1.6** — verdict: none. `AppShell` (was App.tsx body): `ConditionalComponent`
  `shouldMount` gate, the `[...components].sort((a,b)=>order)` render, the
  per-module `<AppErrorBoundary label={comp.id} fallback={()=>null}>`, the
  `data-testid="app-root" className="h-full"` div, and `initSync` in a `useEffect`
  are byte-identical; `initSync(useAuthStore)`→`initSync(authStore)` (prop). The
  thin ziee `App.tsx` preserves the `loadModules()`-at-module-eval boot order.
- **DRIFT-1.7** — verdict: none. `LazyComponentRenderer` (loader-keyed lazy-type
  WeakMap cache, frozen EMPTY_PROPS, is-lazy heuristic, Suspense wrap),
  `Loading`, `DivScrollY` (native-flow branch + merged options/className) are
  verbatim; DivScrollY's `Stores.AppLayout ?? {}` fallback preserved via the seam
  cast. `usePrefetchModules` idle-callback prefetch verbatim.
- **DRIFT-1.8** — verdict: none. Render-identity confirmed live: the gallery
  runtime-health pass over 200 surface/state cells ×2 themes reported **0 gating
  HIGH findings** — every surface (all wrapped by the moved ThemeProvider + error
  boundaries) renders healthy.

**Unresolved drifts:** 0
