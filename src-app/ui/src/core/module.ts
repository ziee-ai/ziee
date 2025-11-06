import type {
  AppModule,
  ModuleMetadata,
  RouteConfig,
  StoreRegistration,
  SidebarRegistration,
  SettingsMenuItem,
  GlobalComponent,
} from './router/types'

export interface CreateModuleOptions {
  metadata: ModuleMetadata
  routes: RouteConfig[]
  stores?: StoreRegistration[]
  sidebar?: SidebarRegistration
  settings?: SettingsMenuItem[]
  globalComponents?: GlobalComponent[]
  initialize?: () => void | Promise<void>
  cleanup?: () => void | Promise<void>
}

export function createModule(options: CreateModuleOptions): AppModule {
  return {
    metadata: options.metadata,
    registerRoutes: () => options.routes,
    registerStores: options.stores ? () => options.stores! : undefined,
    registerSidebar: options.sidebar ? () => options.sidebar! : undefined,
    registerSettings: options.settings ? () => options.settings! : undefined,
    registerGlobalComponents: options.globalComponents ? () => options.globalComponents! : undefined,
    initialize: options.initialize,
    cleanup: options.cleanup,
  }
}
