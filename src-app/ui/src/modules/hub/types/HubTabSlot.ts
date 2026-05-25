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
  refresh: () => Promise<void> // Each tab provides its own refresh logic
}

// Declare the slot in the global Slots interface
declare module '@/core/module-system/types' {
  interface Slots {
    hubTabs: HubTabSlot[]
  }
}

export {}
