/**
 * Desktop Module Loader
 *
 * Loads desktop-specific modules and registers them with the core UI module system
 */

import { useRouterStore, type AppModule } from '@ziee/ui-core'

/**
 * Load desktop modules and register with core UI
 *
 * Desktop modules are just additional AppModules that get loaded
 * alongside the core UI modules. They use the same module system.
 */
export function loadDesktopModules(): void {
  const { registerModule } = useRouterStore.getState()

  // Auto-discover all module.tsx files in desktop modules directory
  const moduleFiles = import.meta.glob<{ default: AppModule }>(
    './**/module.tsx',
    { eager: true },
  )

  // Register each discovered module with the core module system
  for (const [path, moduleExports] of Object.entries(moduleFiles)) {
    const module = moduleExports.default
    if (module) {
      console.log(
        `📦 Loading desktop module: ${module.metadata.name} from ${path}`,
      )
      registerModule(module)
    }
  }
}
