# DECISIONS — smart module loading

### DEC-1: How does a module declare when to load?
**Resolution:** A `shouldLoad?: (ctx: ModuleLoadContext) => boolean` predicate in
the module's `createModule({...})` (single authoring file). The build plugin
statically LIFTS the predicate + route paths into an entry manifest, so it's
evaluated without downloading the body.
**Basis:** user — explicit direction ("a property in the module shouldLoad … bake
that in the build system").

### DEC-2: Default when `shouldLoad` is omitted?
**Resolution:** Always-load (CORE). The 5 boot-critical modules (router, auth,
app, config-client, layout) omit it; every feature module declares one.
**Basis:** convention — backward-compatible + the pre-auth surfaces (setup/login)
depend only on those 5 (confirmed by the module-classification mapping).

### DEC-3: How are permissions referenced in a predicate?
**Resolution:** The generated `Permissions` enum via `ctx.can(Permissions.X)`,
never a literal permission string. The build plugin whitelists the `Permissions`
import and HARD-ERRORS on any other identifier in a lifted `shouldLoad`.
**Basis:** user — "no literal string in the permission, it needs to use Permission enum".

### DEC-4: Gating granularity for mixed-permission modules?
**Resolution:** Per-module. A module whose surfaces are ALL behind one admin
permission is permission-gated (`ctx.can(Permissions.X)`) — a non-holder never
downloads its code. A module mixing user + admin surfaces (mcp, skill, workflow,
memory, …) is auth-gated (`ctx.isAuthenticated`); its admin surfaces stay hidden
by the existing route/slot `permission` gates. Splitting mixed modules for
finer granularity is out of scope.
**Basis:** convention — matches the existing 4-layer permission gating; avoids
over-gating a surface a user should see.

### DEC-5: Does the route-driven net bypass the gate?
**Resolution:** NO. `ensureModuleForPath` checks `isEligible(ctx)` before loading,
so a deep-link to an admin route by a non-holder never force-downloads the admin
module. The route stays unresolved (falls through the guard).
**Basis:** security — the whole point is that admin code never reaches a non-admin.

### DEC-6: Idle prefetch (original ITEM-9)?
**Resolution:** SUBSUMED — not implemented as a separate mechanism. The loader
loads eligible modules EAGERLY on eligibility (login triggers the wave
immediately), not lazily-on-route, because the sidebar/settings slots need their
modules registered to render. So there is nothing left to "prefetch" — eligible =
loaded. (Pre-login prefetch of authenticated bodies was rejected: it re-downloads
what login minimises, and can't know a permission-gated module's eligibility
before auth.)
**Basis:** design — eager-on-eligible is the chosen granularity.

### DEC-7: Desktop parity (ITEM-10)?
**Resolution:** DEFERRED (documented, justified — not silently dropped). Desktop
keeps its working eager-glob loader this round. Reasons: (a) desktop is
single-admin (`multiUserMode=false`) + auto-login, so permission-gating is moot
and the minimal-login-page win doesn't apply; (b) it's a native app — the network
payload smart loading optimises matters far less; (c) the desktop loaders are
coupled to the dev gallery + a unit test + main.tsx's fragile pre-render ordering,
and cannot be Tauri-runtime-verified without the Mac/Windows build hosts. The
build plugin already supports multi-root + a blocklist, so desktop parity is a
small, isolated follow-up when a host is available.
**Basis:** convention/scoping — [[project_crossplatform_build_test_hosts]] (desktop
needs its own host to verify); avoid shipping an unverifiable blind refactor.
