import type { AppModule } from '@/core/router/types'
import { useRouterStore } from '@/core'

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
 * })
 */
export function loadModules(): void {
  const { registerModule, initializeModules } = useRouterStore.getState()

  // Auto-discover all module.tsx files in the modules directory
  const moduleFiles = import.meta.glob<{ default: AppModule }>(
    './**/module.tsx',
    { eager: true },
  )

  // Also discover core modules from components directory
  const coreModuleFiles = import.meta.glob<{ default: AppModule }>(
    '../components/**/module.tsx',
    { eager: true },
  )

  // Register each discovered module
  for (const [path, moduleExports] of Object.entries(moduleFiles)) {
    const module = moduleExports.default
    if (module) {
      console.log(`📦 Loading module: ${module.metadata.name} from ${path}`)
      registerModule(module)
    }
  }

  // Register core modules
  for (const [path, moduleExports] of Object.entries(coreModuleFiles)) {
    const module = moduleExports.default
    if (module) {
      console.log(`📦 Loading core module: ${module.metadata.name} from ${path}`)
      registerModule(module)
    }
  }

  // Initialize all modules after registration
  initializeModules()
}
