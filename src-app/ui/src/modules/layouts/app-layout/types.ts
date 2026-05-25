import type {
  ReactNode,
  ReactElement,
  ComponentType,
  LazyExoticComponent,
} from 'react'
import type { StoreProxy } from '@/core/stores'
import type { useAppLayoutStore } from '@/modules/layouts/app-layout/AppLayout.store'
import type { PermissionExpr } from '@/core/permissions'

// Store type declarations
declare module '@/core/stores' {
  interface RegisteredStores {
    AppLayout: StoreProxy<ReturnType<typeof useAppLayoutStore.getState>>
  }
}

/**
 * Sidebar navigation item
 */
export interface SidebarNavItem {
  id: string
  icon: ReactNode
  label: string
  path: string
  order?: number
  /**
   * Optional permission expression. When set, the entry is hidden
   * from the sidebar for users who don't satisfy it. See
   * `.claude/PERMISSION_GATING.md`.
   */
  permission?: PermissionExpr
}

/**
 * Sidebar tool item (appears in tools section)
 */
export interface SidebarToolItem {
  id: string
  icon: ReactNode
  label: string
  path: string
  order?: number
  /**
   * Optional permission expression. When set, the entry is hidden
   * from the sidebar tools section for users who don't satisfy it.
   * See `.claude/PERMISSION_GATING.md`.
   */
  permission?: PermissionExpr
}

/**
 * Sidebar action button (appears at the top)
 */
export interface SidebarActionItem {
  id: string
  icon: ReactNode
  label: string
  onClick?: () => void
  to?: string
  order?: number
}

/**
 * Sidebar widget item (used for components in recent, bottom, footer sections)
 */
export interface SidebarWidgetItem {
  id: string
  component:
    | ComponentType<any>
    | ReactElement
    | LazyExoticComponent<ComponentType<any>>
    | (() => Promise<{ default: ComponentType<any> }>)
  order: number
}

/**
 * Register AppLayout sidebar slots
 */
declare module '@/core/module-system/types' {
  interface Slots {
    sidebarNavigation: SidebarNavItem[]
    sidebarTools: SidebarToolItem[]
    sidebarPrimaryActions: SidebarActionItem[]
    sidebarContent: SidebarWidgetItem[]
    sidebarBottom: SidebarWidgetItem[]
    sidebarFooter: SidebarWidgetItem[]
  }
}
