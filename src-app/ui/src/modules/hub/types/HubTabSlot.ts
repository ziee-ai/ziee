import type { ReactNode } from 'react'
import type { PermissionExpr } from '@/core/permissions'

export interface HubTabSlot {
  id: string
  label: string
  icon: ReactNode
  component: ReactNode | (() => Promise<{ default: React.ComponentType }>)
  order: number
  /**
   * Permissions for the tab. `read` gates whether the tab appears
   * at all (sidebar segmented control + dropdown). `refresh` gates
   * whether the page-level Refresh button is shown when this tab
   * is the active one. See `.claude/PERMISSION_GATING.md`.
   */
  permissions: {
    read: PermissionExpr
    refresh?: PermissionExpr
  }
  /**
   * Optional dynamic gate evaluated alongside `permissions.read`.
   * Returns `true` to render the tab, `false` to hide it. Use for
   * gates that depend on runtime configuration (e.g. an admin
   * policy) rather than only on the user's permissions. The
   * callback is invoked at render time, so it can read from any
   * store proxy.
   */
  shouldRender?: () => boolean
  refresh: () => Promise<void> // Each tab provides its own refresh logic
}

// Declare the slot in the global Slots interface
declare module '@/core/module-system/types' {
  interface Slots {
    hubTabs: HubTabSlot[]
  }
}

export {}
