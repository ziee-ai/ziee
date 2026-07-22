import { manifest } from 'virtual:ziee-module-manifest'
import { useModuleSystemStore } from '@ziee/framework'
import {
  entryForPath,
  isEligible,
  orderByDependencies,
  type ModuleManifestEntry,
} from '@ziee/framework/module-system'
import { useAuthStore } from '@/modules/auth/Auth.store'
import { useAppStore } from '@/modules/app/stores/app'
import { buildLoadContext } from '@/modules/loadContext'

/**
 * SMART module loader.
 *
 * Modules are NOT eagerly discovered anymore. The build plugin
 * (`vite-plugin-module-manifest`) bakes a cheap manifest into the entry —
 * `{ name, shouldLoad?, routePaths, dependencies, load }` per module — and this
 * loader decides, from the current auth/permission/platform context, WHICH
 * module bodies to download + register:
 *
 *  - **Boot:** register every CORE module (no `shouldLoad`) plus any module
 *    whose predicate already passes (e.g. a restored authenticated session).
 *    `initializeModules()` runs once, after this first wave.
 *  - **Reactive:** on login / permission grant / setup completion, re-evaluate
 *    the not-yet-loaded manifest and register the newly-eligible modules as a
 *    second wave (register + per-module `initialize()`, NOT a second
 *    `initializeModules()` — mirrors the desktop second-wave precedent). Modules
 *    are never unloaded; `loaded` makes every wave idempotent.
 *  - **Route-driven:** `ensureModuleForPath()` lets the router pull a module a
 *    deep-link needs even if its predicate hasn't fired yet.
 *
 * A permission-gated predicate (`ctx.can(Permissions.X)`) means the module's
 * code never reaches a user who lacks the permission.
 */

const loaded = new Set<string>()
// Modules whose body download / registration is CURRENTLY in flight. A name is
// added to `loaded` synchronously (to dedupe concurrent waves) BEFORE its chunk
// finishes downloading, so `loaded.has` alone can't tell "settled" from
// "still-arriving". `inFlight` closes that window: a module is settled (its
// routes are registered) only when loaded.has(name) && !inFlight.has(name).
// The router's deep-link fallback uses this to WAIT (render Loading) instead of
// redirecting while the owning module is still on the wire.
const inFlight = new Set<string>()
let coreInitialized = false
let subscribed = false

/** Register a set of manifest entries (dependency-ordered), loading bodies in parallel. */
async function registerWave(entries: ModuleManifestEntry[]): Promise<void> {
  const fresh = entries.filter(e => !loaded.has(e.name))
  if (fresh.length === 0) return
  // Claim names up-front so concurrent waves don't double-load the same module.
  // Mark them in-flight until their routes/slots are actually registered below.
  fresh.forEach(e => {
    loaded.add(e.name)
    inFlight.add(e.name)
  })

  const ordered = orderByDependencies(fresh)
  const bodies = await Promise.all(
    ordered.map(async e => {
      try {
        return { e, mod: (await e.load()).default }
      } catch (err) {
        loaded.delete(e.name) // allow a retry on the next wave
        inFlight.delete(e.name)
        console.error(`[loader] failed to load module "${e.name}"`, err)
        return null
      }
    }),
  )

  const { registerModule, initializeModules } = useModuleSystemStore.getState()
  const registered = bodies.filter(
    (b): b is { e: ModuleManifestEntry; mod: import('@ziee/framework').AppModule } =>
      b !== null && !!b.mod,
  )

  for (const { mod } of registered) {
    // registerModule fans out onModuleRegister to already-registered modules
    // (router collects routes, sidebar collects slots) — do NOT hand-roll a
    // second fan-out loop (that double-registers routes).
    registerModule(mod)
  }
  // Routes/slots are now registered — these modules are settled.
  fresh.forEach(e => inFlight.delete(e.name))

  if (!coreInitialized) {
    // First wave: full framework init (registers core EventBus/ModuleSystem
    // stores + rebuilds slots + runs each module's initialize()).
    coreInitialized = true
    initializeModules()
  } else {
    // Later waves: initialize only the new modules; do NOT re-run
    // initializeModules() (it rebuilds slots from scratch — the second-wave
    // pattern from desktop-loader avoids that).
    for (const { mod } of registered) {
      if (mod.initialize) Promise.resolve().then(() => mod.initialize!())
    }
  }
}

/** Evaluate the manifest against the current context and register newly-eligible modules. */
async function loadEligible(): Promise<void> {
  const ctx = buildLoadContext(
    typeof window !== 'undefined' ? window.location.pathname : '/',
  )
  await registerWave(manifest.filter(e => isEligible(e, ctx)))
}

/** Subscribe to the core stores so login / permission grant / setup-done loads more modules. */
function subscribeReactive(): void {
  if (subscribed) return
  subscribed = true
  const reeval = () => {
    void loadEligible()
  }
  useAuthStore.subscribe(reeval)
  useAppStore.subscribe(reeval)
}

/**
 * Boot entry point (called from App.tsx). Registers the first wave (core + any
 * already-eligible) and wires the reactive re-evaluation. Resolves once the
 * first wave is registered — that's what `AppShell` gates first paint on.
 */
export async function loadModules(): Promise<void> {
  await loadEligible()
  subscribeReactive()
}

/**
 * Route-driven safety net: ensure the module OWNING `pathname` is loaded (e.g. a
 * deep-link to a page whose predicate hasn't fired, or a just-granted
 * permission). Returns true if it triggered/awaited a load, false if no manifest
 * entry owns the path (a genuine 404) or it was already loaded.
 */
export async function ensureModuleForPath(pathname: string): Promise<boolean> {
  const entry = entryForPath(manifest, pathname)
  if (!entry || loaded.has(entry.name)) return false
  // SECURITY: the route-driven net must NOT bypass the gate — only load a module
  // the current context is eligible for. Otherwise a user lacking a permission
  // could force-download an admin module's code just by navigating to its route.
  // An ineligible target simply stays unresolved (the route falls through the
  // guard/404), exactly as if the module didn't exist for this user.
  if (!isEligible(entry, buildLoadContext(pathname))) return false
  await registerWave([entry])
  return true
}

/**
 * Deep-link fallback signal: is `pathname` owned by an eligible manifest module
 * that has NOT yet finished loading? When true, the router must render a loading
 * state instead of redirecting — the module's routes are on the wire and will
 * appear on the next render (it arrived before its reactive load wave, e.g. a
 * hard-reload / bookmark straight onto a lazy-module route).
 *
 * Returns false when: no manifest entry owns the path (genuine 404), the user
 * isn't eligible for the owner (unauthorized — same as if it didn't exist), or
 * the owner is already settled (loaded AND its routes registered) — in which
 * case an unmatched path is a real 404 and the fallback should redirect.
 */
export function isPathModulePending(pathname: string): boolean {
  const entry = entryForPath(manifest, pathname)
  if (!entry) return false
  if (!isEligible(entry, buildLoadContext(pathname))) return false
  return !loaded.has(entry.name) || inFlight.has(entry.name)
}

/** Names of every module registered so far (for prefetch/debug). */
export function loadedModuleNames(): string[] {
  return [...loaded]
}

/** The full manifest (for idle prefetch). */
export { manifest }
