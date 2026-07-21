# PLAN — smart module loading

Build-extracted, per-module `shouldLoad` predicate that gates whether a module's
FULL body (routes, slots, stores, components) is downloaded + registered. The
build system extracts each module's cheap decision layer (`{name, shouldLoad,
routePaths, dependencies}`) into a manifest baked in the entry; the heavy body is
a separate chunk loaded only when `shouldLoad(ctx)` passes. Gating conditions:
auth state, **user permission (via the `Permissions` enum — never a literal
string)**, platform, and (as a safety net) the navigated route.

## Goal / definition of done

A first-run **setup page** and the **login page** download ONLY the core modules
(router, auth, app, config-client) + shell — not chat/file/hub/llm/mcp/etc.
After login, only modules the user's `shouldLoad` allows register (an admin-only
module's code never reaches a non-admin). Deep-links and permission grants still
resolve. Web + desktop share one manifest model.

## Items

- **ITEM-1**: `ModuleLoadContext` type + a non-reactive `can(...perms)` permission helper. `ctx = { isAuthenticated, needsSetup, path, permissions: string[], platform: 'web'|'desktop', can(...perms): boolean }`. `can` reuses the existing permission evaluation (is_admin wildcard short-circuit) from `core/permissions` (`evaluatePermission`/`hasPermissionNow`) so a module predicate reads `ctx.can(Permissions.UsersRead)`.
- **ITEM-2**: Add `shouldLoad?: (ctx: ModuleLoadContext) => boolean` to `CreateModuleOptions` (`sdk/.../module.ts`) and the runtime `AppModule` (`module-system/types.ts`); `createModule` carries it through. Omitted ⇒ treated as always-load (core). (Default semantics locked in DEC-1.)
- **ITEM-3**: Vite plugin `vite-plugin-module-manifest` — globs `modules/**/module.tsx` + `components/**/module.tsx`, statically extracts per module `{ name, shouldLoad (source-lifted arrow fn), routePaths (the `path:` string literals from `routes`), dependencies }`, and emits a virtual module `virtual:ziee-module-manifest` = an array of `{ name, shouldLoad, routePaths, dependencies, load: () => import('<abs module path>') }`. The lifted `shouldLoad` may reference ONLY `ctx` and the whitelisted `Permissions` import (hoisted into the manifest); ANY other free identifier / import in `shouldLoad` is a hard BUILD ERROR (the purity constraint that makes lifting safe).
- **ITEM-4**: Manifest-driven `loadModules` (web `loader.ts` rewrite) — build `ctx`, evaluate `shouldLoad(ctx)` for every manifest entry, topologically sort the eligible set by `dependencies`, dynamically `load()` each body + `registerModule` it. The always-load (no-`shouldLoad`) core set is what `modulesReady` awaits. Replaces the eager/lazy `import.meta.glob` discovery.
- **ITEM-5**: Reactive load-on-eligible — the loader subscribes to the Auth store; when `isAuthenticated` / `permissions` / `needsSetup` change, it re-evaluates the NOT-yet-loaded manifest entries and loads the newly-eligible ones (idempotent per module; modules are never unloaded). Covers login and mid-session permission grant (group edit → `sync:Session` → `/auth/me` refetch).
- **ITEM-6**: Route-driven safety net — a router-level hook that, on navigation to a path NO currently-loaded module owns, matches the path against manifest `routePaths` and loads the owning module before the route resolves (deep-link to a not-yet-eligible page; a just-granted permission). Falls through to the existing 404 only if no manifest entry matches.
- **ITEM-7**: Boot/registration gating — `AppShell` gates first paint on `modulesReady` = core registered (routes/slots are already reactive, so feature modules populate the shell as they register); the authenticated shell shows the existing `Loading` affordance during the brief post-login registration if bodies aren't prefetched yet. No empty-shell flash, no pill/nav flash.
- **ITEM-8**: Annotate every module's `shouldLoad`. Core (`router`, `auth`, `app`, `config-client`, core `components/**`) omit it. Authenticated-only modules → `ctx => ctx.isAuthenticated`. Permission-scoped modules → `ctx => ctx.isAuthenticated && ctx.can(Permissions.X)` using the enum (e.g. `user`→`UsersRead`, `mcp` admin surfaces, `code-sandbox`, `hardware`, `llm-*` admin, `voice`, `summarization`, `auth-providers`, `server-update`, `js-tool`, `file-rag`, `memory` admin, `web-search` admin, `skill` admin, `workflow` admin — matched to each module's route/slot permission).
- **ITEM-9**: Idle prefetch — after core boot, prefetch (not register) the module bodies that will become eligible (repurpose `usePrefetchModules` to walk the manifest) during idle, so post-login registration is instant. Prefetch respects `shouldLoad` where statically knowable (auth-gated yes; permission-gated only after auth).
- **ITEM-10**: Desktop parity — `loader.desktop.ts` (core-blocklist fork) + `desktop-loader.ts` (desktop-own second wave) consume the SAME manifest; `platform:'desktop'` in ctx; honor `CORE_MODULE_BLOCKLIST`; desktop auto-login drives the ITEM-5 re-eval; preserve `main.tsx`'s pre-render `setMultiUserMode` ordering (core modules still register synchronously enough before that call, or move it behind core-ready).
- **ITEM-11**: `addRoutes` dedup — make the Routes store idempotent (dedup by `path`+`layout`) so staged/re-entrant registration can't double-insert routes (the pre-existing double-`onModuleRegister` bug, which staged loading would compound).
- **ITEM-12**: Non-reactive extension-registry correctness — verify feature-local registries (`ProjectExtensionRegistry`, chat registry) are populated by their module body BEFORE their consumer renders (they load together), and that no pre-auth surface reads a registry a deferred module feeds. (Chat extensions already deferred+gated on this branch.)

## Files to touch

- `sdk/packages/framework/src/module.ts` — `CreateModuleOptions.shouldLoad`, carry-through (ITEM-2).
- `sdk/packages/framework/src/module-system/types.ts` — `AppModule.shouldLoad`, `ModuleLoadContext`, `routePaths` typing (ITEM-1/2).
- `sdk/packages/framework/src/module-system/manifest.ts` (new) — manifest entry type + the eligibility/topo-sort helpers, framework-agnostic (ITEM-4).
- `src-app/ui/vite/module-manifest-plugin.ts` (new) — the Vite plugin (ITEM-3).
- `src-app/ui/vite.config.ts` (+ `vite.config.preview.ts` inherits) — register the plugin (ITEM-3).
- `src-app/ui/src/modules/loader.ts` — manifest-driven web loader (ITEM-4/5/6).
- `src-app/ui/src/modules/loadContext.ts` (new) — build `ModuleLoadContext` + `can()` from Auth store (ITEM-1).
- `src-app/ui/src/App.tsx` — wire loader + `modulesReady` (ITEM-4/7).
- `sdk/packages/shell/src/bootstrap/AppShell.tsx` — registration gating (ITEM-7).
- `src-app/ui/src/modules/router/stores/routes-store.ts` — `addRoutes` dedup (ITEM-11).
- `src-app/ui/src/modules/router/components/RouterComponent.tsx` — route-driven load hook (ITEM-6).
- `src-app/ui/src/modules/*/module.tsx` (~40) — `shouldLoad` annotations (ITEM-8).
- `src-app/ui/src/modules/loader.desktop.ts`, `src-app/desktop/ui/src/modules/desktop-loader.ts`, `src-app/desktop/ui/src/main.tsx` — desktop parity (ITEM-10).
- `sdk/packages/shell/src/hooks/usePrefetchModules.ts` — manifest-driven idle prefetch (ITEM-9).

## Patterns to follow

- **Vite plugin**: mirror the existing in-repo vite plugins (`src-app/ui/vite.config.ts`'s `localOverridePlugin` and any `src-app/ui/vite/*`) for structure, virtual-module id convention (`resolveId`/`load`), and the `import.meta.glob`-vs-manifest handoff. Use `es-module-lexer`/`@babel/parser` (already transitive via vite) for the static extraction; keep the extractor pure + unit-tested.
- **Loader**: mirror the current `loader.ts` topo-sort (`resolveDependencies`) + `registerModule` loop and the `desktop-loader.ts` "second wave, don't re-call `initializeModules`" precedent for staged registration.
- **Permission `can()`**: mirror `core/permissions`' `hasPermissionNow` / `evaluatePermission` (is_admin wildcard, `Permissions` enum), NOT a hand-rolled string check.
- **Boot gating**: mirror the `AppShell` `modulesReady` gate + `AuthGuard` phase machine already present on this branch.
- **Loading affordance**: reuse the shell `Loading` component (fullscreen) — no bespoke spinner.

## UI-surface checklist

This feature adds NO new page/drawer/card. The only user-visible surface is
**loading/registration states**:
- **Precedent**: reuse the existing `Loading` fullscreen (AppShell placeholder / AuthGuard `checking` state) — no new component.
- **Responsive**: the loader is a centered fullscreen affordance; trivially responsive at 390px/desktop (mirrors `AuthGuard`'s `Loading`).
- **User-visible progress**: post-login registration should be effectively invisible (idle-prefetch, ITEM-9); if a body is still loading, the existing route-level Suspense/`Loading` covers it. No silent hang — the route fallback renders while a body streams.
- **Populated render / JTBD**: the "job" is *nothing to see* — modules the user can't use never appear; the ones they can appear as before. The DoD is measured by the boot-payload (setup/login ≈ core only) + no regression in the authenticated app's surfaces.
- **Platform**: `ctx.platform` gates desktop-only vs web-only behavior via a pure predicate; no redundant chrome added.
