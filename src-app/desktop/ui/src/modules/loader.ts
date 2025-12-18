/**
 * Desktop Module Loader
 *
 * Loads desktop-specific modules and registers them with the core UI module system
 */

import { useModuleSystemStore, type AppModule } from '@ziee/ui-core'

/**
 * Load desktop modules and register with core UI
 *
 * Desktop modules are loaded AFTER core modules, so we need to:
 * 1. Register each module
 * 2. Manually initialize them (since initializeModules() already ran for core modules)
 */
export function loadDesktopModules(): void {
  const { registerModule } = useModuleSystemStore.getState()

  // Auto-discover all module.tsx files in desktop modules directory
  const moduleFiles = import.meta.glob<{ default: AppModule }>(
    './**/module.tsx',
    { eager: true },
  )

  // Collect modules for initialization
  const desktopModules: AppModule[] = []

  // Register each discovered module with the core module system
  for (const [path, moduleExports] of Object.entries(moduleFiles)) {
    const module = moduleExports.default
    if (module) {
      console.log(
        `📦 Loading desktop module: ${module.metadata.name} from ${path}`,
      )
      registerModule(module)
      desktopModules.push(module)
    }
  }

  // Initialize desktop modules (since they were registered after initializeModules() ran)
  console.log('🚀 Initializing desktop modules...')
  for (const module of desktopModules) {
    if (module.initialize) {
      try {
        const result = module.initialize()
        if (result instanceof Promise) {
          result.catch(error => {
            console.error(
              `Failed to initialize desktop module ${module.metadata.name}:`,
              error,
            )
          })
        }
      } catch (error) {
        console.error(
          `Failed to initialize desktop module ${module.metadata.name}:`,
          error,
        )
      }
    }
  }
}
