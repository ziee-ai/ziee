import { AppLayout } from '@/modules/layouts/app-layout'
import SettingsPage from '@/modules/settings/SettingsPage'

/**
 * The settings shell = the app shell (`AppLayout`) wrapping the settings nav
 * page. Kept in its OWN module so `SettingsLayoutDef` can reference it lazily —
 * importing the def (every settings module + auth do) must NOT drag the whole
 * shell (HeaderBarContainer, LeftSidebar, …) into the boot payload, since the
 * settings routes are all authenticated.
 */
export function SettingsLayoutView() {
  return (
    <AppLayout>
      <SettingsPage />
    </AppLayout>
  )
}

export default SettingsLayoutView
