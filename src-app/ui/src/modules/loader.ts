import type { AppModule } from '@ziee/framework/module-system/types'
import { useModuleSystemStore } from '@ziee/framework'

/**
 * Auto-discovers and registers all modules from the modules directory.
 *
 * To add a new module:
 * 1. Create a new directory under src/modules/
 * 2. Create a module.tsx file that exports a default module
 * 3. The module will be automatically discovered and registered
 *
 * Example module structure:
 * src/modules/mymodule/module.tsx:
 *
 * export default createModule({
 *   metadata: { name: 'mymodule', version: '1.0.0' },
 *   routes: [...],
 *   stores: [...],
 *   dependencies: ['router', 'auth'],  // Optional: modules to load first
 * })
 */

/**
 * Phase 1: Meta-Framework - Dependency resolution
 * Sorts modules in dependency order using topological sort
 */
function resolveDependencies(modules: AppModule[]): AppModule[] {
  const graph = new Map<string, string[]>()
  const moduleMap = new Map<string, AppModule>()

  // Build dependency graph
  modules.forEach(module => {
    const deps = module.registerDependencies?.() || []
    graph.set(module.metadata.name, deps)
    moduleMap.set(module.metadata.name, module)
  })

  // Topological sort using DFS
  const sorted: string[] = []
  const visited = new Set<string>()
  const visiting = new Set<string>() // For cycle detection

  function visit(name: string, path: string[] = []) {
    if (visited.has(name)) return

    // Detect circular dependencies
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

  // Visit all modules
  graph.forEach((_, name) => visit(name))

  // Return modules in dependency order
  return sorted.map(name => moduleMap.get(name)!)
}

export async function loadModules(): Promise<void> {
  const { registerModule, initializeModules } = useModuleSystemStore.getState()

  // Auto-discover all module.tsx files in the modules directory.
  // LAZY glob (no `{ eager: true }`): each module.tsx becomes its OWN chunk
  // instead of being inlined into the entry chunk. Since a module.tsx statically
  // imports its stores/components to wire routes+slots, an EAGER glob dragged
  // every module's whole store/component subtree into entry (O(all-modules) boot
  // weight). Lazy-globbing splits each module into a separate chunk; we still
  // load them ALL at boot (below) — modules must be registered before routing
  // renders — but as parallel, individually-cacheable chunks, and the entry
  // chunk no longer carries them.
  const moduleFiles = import.meta.glob<{ default: AppModule }>(
    './**/module.tsx',
  )

  // Also discover core modules from components directory
  const coreModuleFiles = import.meta.glob<{ default: AppModule }>(
    '../components/**/module.tsx',
  )

  // Collect all modules (load every discovered chunk in parallel)
  const loaders = { ...moduleFiles, ...coreModuleFiles }
  const loaded = await Promise.all(
    Object.values(loaders).map(load => load()),
  )
  const allModules: AppModule[] = []
  for (const moduleExports of loaded) {
    const module = moduleExports.default
    if (module) {
      allModules.push(module)
    }
  }

  // Phase 1: Meta-Framework - Sort by dependencies
  const sortedModules = resolveDependencies(allModules)

  // Register modules in dependency order
  for (const module of sortedModules) {
    registerModule(module)

    // Phase 1: Meta-Framework - Call onModuleRegister hooks on all previously registered modules
    const { modules: registeredModules } = useModuleSystemStore.getState()
    registeredModules.forEach(registeredModule => {
      if (
        registeredModule.metadata.name !== module.metadata.name &&
        registeredModule.onModuleRegister
      ) {
        registeredModule.onModuleRegister(module)
      }
    })

    // Also call the new module's hook for all previously registered modules
    if (module.onModuleRegister) {
      registeredModules.forEach(registeredModule => {
        if (registeredModule.metadata.name !== module.metadata.name) {
          module.onModuleRegister!(registeredModule)
        }
      })
    }
  }

  // Initialize all modules after registration
  initializeModules()
}
