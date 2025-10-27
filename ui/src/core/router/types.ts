import type { ReactElement } from 'react'
import type { UseBoundStore, StoreApi } from 'zustand'

export interface RouteConfig {
  path: string
  element: ReactElement
  requiresAuth?: boolean
  layout?: 'default' | 'none'
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

export interface AppModule {
  metadata: ModuleMetadata
  registerRoutes: () => RouteConfig[]
  registerStores?: () => StoreRegistration[]
  initialize?: () => void | Promise<void>
  cleanup?: () => void | Promise<void>
}
