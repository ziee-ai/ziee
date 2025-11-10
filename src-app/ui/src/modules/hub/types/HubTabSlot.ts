import type { ReactNode } from 'react'

export interface HubTabSlot {
  id: string
  label: string
  icon: ReactNode
  component: ReactNode | (() => Promise<{ default: React.ComponentType }>)
  order: number
  permission?: string
  refresh: () => Promise<void> // Each tab provides its own refresh logic
}

// Declare the slot in the global Slots interface
declare module '@/core/module-system/types' {
  interface Slots {
    hubTabs: HubTabSlot[]
  }
}

export {}
