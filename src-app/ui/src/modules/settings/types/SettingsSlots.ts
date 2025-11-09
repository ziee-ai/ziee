import type { ReactNode } from 'react'

export interface SettingsPageSlot {
  id: string
  icon: ReactNode
  label: string
  path: string
  order: number
}

// Extend the global Slots interface
declare module '@/core/module-system/types' {
  interface Slots {
    settingsUserPages: SettingsPageSlot[]
    settingsAdminPages: SettingsPageSlot[]
  }
}

export {}
