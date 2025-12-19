import { AppLayout } from '@/modules/layouts/app-layout'
import type { LayoutDefinition } from '@/modules/router/types'
import SettingsPage from '@/modules/settings/SettingsPage'

export function SettingsLayout() {
  return (
    <AppLayout>
      <SettingsPage />
    </AppLayout>
  )
}

export const SettingsLayoutDef: LayoutDefinition<undefined> = {
  component: SettingsLayout as any,
  mergeOptions: () => undefined,
}

// Default export for backwards compatibility
export default SettingsLayout
