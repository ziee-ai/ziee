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

export interface AppModule {
  metadata: ModuleMetadata
  registerRoutes: () => RouteConfig[]
  registerStores?: () => StoreRegistration[]
  registerSidebar?: () => SidebarRegistration
  registerSettings?: () => SettingsMenuItem[]
  initialize?: () => void | Promise<void>
  cleanup?: () => void | Promise<void>
}
