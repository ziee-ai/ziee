import type { AppModule, ModuleMetadata, RouteConfig, StoreRegistration, SidebarRegistration } from './router/types'

export interface CreateModuleOptions {
  metadata: ModuleMetadata
  routes: RouteConfig[]
  stores?: StoreRegistration[]
  sidebar?: SidebarRegistration
  initialize?: () => void | Promise<void>
  cleanup?: () => void | Promise<void>
}

export function createModule(options: CreateModuleOptions): AppModule {
  return {
    metadata: options.metadata,
    registerRoutes: () => options.routes,
    registerStores: options.stores ? () => options.stores! : undefined,
    registerSidebar: options.sidebar ? () => options.sidebar! : undefined,
    initialize: options.initialize,
    cleanup: options.cleanup,
  }
}
