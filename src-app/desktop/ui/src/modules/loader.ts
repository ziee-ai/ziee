/**
 * Desktop replacement for `ui/src/modules/loader.ts`.
 *
 * Resolved by Vite's localOverridePlugin: App.tsx imports
 * `loadModules` from `@/modules/loader`, and inside the desktop
 * bundle that `@/` alias hits this file before falling through to
 * core. App.tsx then calls `loadModules()` at the top of the file
 * (module-init side effect) — that's the single entry point for
 * registering all core modules in the desktop build.
 *
 * Why fork the loader instead of stubbing a widget or scrubbing
 * slots post-load: core's `loadModules` uses `import.meta.glob` to
 * auto-discover modules, and globs aren't resolved through Vite's
 * resolver chain — `localOverridePlugin` can intercept imports but
 * not filesystem globs. So a `desktop/ui/src/modules/user-profile/
 * module.tsx` would never be discovered (wrong glob root) and the
 * core file would still register. The fork is the right seam: glob
 * the core paths explicitly and skip what doesn't belong on desktop.
 *
 * Don't add desktop-only modules here — they live under
 * `desktop/ui/src/modules/` and are loaded by `desktop-loader.ts`.
 * This file is strictly "the core loader with a blocklist."
 */

import type { AppModule } from '@/core/module-system/types'
import { useModuleSystemStore } from '@/core'

/**
 * Core module names to skip on desktop. Identified by
 * `module.metadata.name` (not by path) so it survives directory
 * renames. Each entry needs a short reason.
 */
const CORE_MODULE_BLOCKLIST = new Set<string>([
  // No multi-user concept on desktop — auto_login + single admin.
  // Showing a profile chip in the sidebar implies account-switching
  // that doesn't exist. Dropping the module also drops its
  // sidebarFooter slot entry, so the footer divider stays invisible.
  'user-profile',
])

function resolveDependencies(modules: AppModule[]): AppModule[] {
  const graph = new Map<string, string[]>()
  const moduleMap = new Map<string, AppModule>()

  modules.forEach(module => {
    const deps = module.registerDependencies?.() || []
    graph.set(module.metadata.name, deps)
    moduleMap.set(module.metadata.name, module)
  })

  const sorted: string[] = []
  const visited = new Set<string>()
  const visiting = new Set<string>()

  function visit(name: string, path: string[] = []) {
    if (visited.has(name)) return

    if (visiting.has(name)) {
      const cycle = [...path, name].join(' -> ')
      throw new Error(`Circular dependency detected: ${cycle}`)
    }

    visiting.add(name)

    const deps = graph.get(name) || []
    deps.forEach(dep => {
      if (!moduleMap.has(dep)) {
        console.warn(
          `Module "${name}" depends on "${dep}" which is not loaded. Skipping dependency.`,
        )
        return
      }
      visit(dep, [...path, name])
    })

    visiting.delete(name)
    visited.add(name)
    sorted.push(name)
  }

  graph.forEach((_, name) => visit(name))

  return sorted.map(name => moduleMap.get(name)!)
}

export function loadModules(): void {
  const { registerModule, initializeModules } = useModuleSystemStore.getState()

  // Glob the CORE module paths explicitly via the @ziee/ui-core
  // alias (resolves to `../../ui/src` per vite.config). Vite honors
  // aliases inside `import.meta.glob` patterns — verified by the
  // existing `desktop-loader.ts` pattern.
  const moduleFiles = import.meta.glob<{ default: AppModule }>(
    '@ziee/ui-core/modules/**/module.tsx',
    { eager: true },
  )
  const coreModuleFiles = import.meta.glob<{ default: AppModule }>(
    '@ziee/ui-core/components/**/module.tsx',
    { eager: true },
  )

  const allModules: AppModule[] = []

  for (const [path, moduleExports] of Object.entries(moduleFiles)) {
    const module = moduleExports.default
    if (module && !CORE_MODULE_BLOCKLIST.has(module.metadata.name)) {
      allModules.push(module)
    } else if (module) {
      console.log(`📦 Desktop: skipping blocklisted core module "${module.metadata.name}" (${path})`)
    }
  }

  for (const [path, moduleExports] of Object.entries(coreModuleFiles)) {
    const module = moduleExports.default
    if (module && !CORE_MODULE_BLOCKLIST.has(module.metadata.name)) {
      allModules.push(module)
    } else if (module) {
      console.log(`📦 Desktop: skipping blocklisted core module "${module.metadata.name}" (${path})`)
    }
  }

  const sortedModules = resolveDependencies(allModules)

  for (const module of sortedModules) {
    registerModule(module)

    const { modules: registeredModules } = useModuleSystemStore.getState()
    registeredModules.forEach(registeredModule => {
      if (
        registeredModule.metadata.name !== module.metadata.name &&
        registeredModule.onModuleRegister
      ) {
        registeredModule.onModuleRegister(module)
      }
    })

    if (module.onModuleRegister) {
      registeredModules.forEach(registeredModule => {
        if (registeredModule.metadata.name !== module.metadata.name) {
          module.onModuleRegister!(registeredModule)
        }
      })
    }
  }

  initializeModules()
}
