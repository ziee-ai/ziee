import type { ReactElement, ComponentType, ReactNode, LazyExoticComponent } from 'react'
import type { UseBoundStore, StoreApi } from 'zustand'

export interface RouteConfig {
  path: string
  element: ReactElement | LazyExoticComponent<ComponentType<any>> | (() => Promise<{ default: ComponentType<any> }>)
  requiresAuth?: boolean
  layout?: ComponentType<{ children: ReactNode }>
  index?: boolean
}

export interface StoreRegistration {
  name: string
  store: UseBoundStore<StoreApi<any>>
}

export interface ModuleMetadata {
  name: string
  version: string
  description?: string
}

// Sidebar types
export interface SidebarActionButton {
  id: string
  icon: ReactNode
  label: string
  onClick?: () => void
  to?: string
  order?: number
}

export interface SidebarNavItem {
  id: string
  icon: ReactNode
  label: string
  path: string
  order?: number
  requiresPermission?: string
}

export interface SidebarWidget {
  id: string
  slot: string
  component: ReactNode
  order?: number
}

export interface SidebarRegistration {
  primaryActions?: SidebarActionButton[]
  navigation?: SidebarNavItem[]
  tools?: SidebarNavItem[]
  widgets?: SidebarWidget[]
}

// Settings types
export interface SettingsMenuItem {
  id: string
  icon: ReactNode
  label: string
  path: string
  section: 'user' | 'admin'
  order?: number
}

// Global Components types
export interface GlobalComponent {
  id: string                          // Unique identifier (e.g., 'model-download-drawer')
  component: ReactElement | LazyExoticComponent<ComponentType<any>> | (() => Promise<{ default: ComponentType<any> }>)  // Lazy or eager component
  order?: number                      // Optional: Mount order (default: 0)
}

// Slot types - extensible slot system with declaration merging
// Modules can declare slots using declaration merging:
// declare module '@/core/router/types' {
//   interface Slots {
//     userGroup: GroupWidget[]
//   }
// }
export interface Slots {}

export type SlotRegistration = Partial<Slots>

export interface AppModule {
  metadata: ModuleMetadata
  registerRoutes: () => RouteConfig[]
  registerStores?: () => StoreRegistration[]
  registerSidebar?: () => SidebarRegistration
  registerSettings?: () => SettingsMenuItem[]
  registerGlobalComponents?: () => GlobalComponent[]
  registerSlots?: () => SlotRegistration
  initialize?: () => void | Promise<void>
  cleanup?: () => void | Promise<void>
}
