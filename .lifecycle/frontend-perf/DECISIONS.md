# frontend-perf — DECISIONS

### DEC-1: How are the Streamdown plugins deferred without changing the rendered output?
**Resolution:** Add a `variant: 'chat' | 'base'` prop to the existing
`LazyStreamdown` wrapper. Each variant has its own `lazyWithPreload` loader whose
async factory `import()`s BOTH `streamdown` and the matching plugin module
(`chatMarkdownPlugins` for chat — adds the HtmlBlock renderer; `streamdownPlugins`
for base) and returns a component that injects `plugins`. Call sites pass `variant`
and stop statically importing the plugin objects. The `shikiTheme` prop is dropped
at call sites — the `@streamdown/code` plugin carries its own themes and ignores it
(documented in `streamdownPlugins.ts`), so removing it is a no-op for rendering.
**Basis:** codebase — mirrors the existing `LazyStreamdown` lazy pattern and its
desktop-preload contract; the only two plugin variants in use are chat vs base.

### DEC-2: Does `usePrefetchModules` prefetch nothing, or the post-login landing route, when logged out?
**Resolution:** Prefetch NOTHING when unauthenticated; when authenticated, prefetch
only routes the user is permitted to reach, excluding the current route. Drop the
forced `{timeout: 2000}` so it runs at true idle.
**Basis:** convention — matches the route guards' `hasPermissionNow`/auth gate; a
logged-out user can navigate to no protected route, so prefetching them is pure
waste + the exact symptom observed.

### DEC-3: Fixed constant vs admin-configurable for the perf-budget thresholds (ITEM-8)?
**Resolution:** Fixed constants in the committed budget script — NOT an
admin-configurable settings row. These are a BUILD-TIME developer guardrail
(regression fence), not a runtime deployment tunable; there is no operator reason
to change them at runtime and no server surface involved.
**Basis:** convention — mirrors the other build-time `check:*` gates
(`check:design-spec`, `check:testid-registry`) which are fixed-threshold scripts.

### DEC-4: Do these changes need a desktop `ui/` override diff (rule R2-3)?
**Resolution:** Per item. ITEM-1/5/7 are in `src-app/ui/**` shared code with no
security-relevant logic and no existing desktop override → no desktop edit. ITEM-2
touches `sdk/packages/shell` (shared by both) — verify the desktop loader's
name-blocklist interaction and diff `loader.desktop.ts` before shipping ITEM-3.
**Basis:** codebase — `lazyWithPreload.desktop.ts` is the only relevant existing
override and its contract is preserved by routing every loader through
`lazyWithPreload`.
