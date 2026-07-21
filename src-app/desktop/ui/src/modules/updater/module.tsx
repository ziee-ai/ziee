/**
 * Desktop Auto-Updater module.
 *
 * Owns the user-facing About page at `/settings/about` (current version +
 * check / download / install for application updates). Registered ONLY in the
 * desktop bundle — updates are a native-app concern.
 *
 * Backend control surface: the desktop crate's aide-documented
 * `/api/desktop/updater/*` routes, driven through `Stores.Updater`.
 */

import { createModule, type AppModule } from '@ziee/ui-core'
import { lazy } from 'react'
import { Info } from 'lucide-react'
import { SettingsLayoutDef } from '@ziee/ui-core/modules/settings/SettingsLayout'

import { useUpdaterStore } from '@ziee/desktop/modules/updater/stores/updater'
import { UpdateBanner } from './components/UpdateBanner'

const AboutPage = lazy(() =>
  import('./pages/AboutPage').then((m) => ({
    default: m.AboutPage,
  })),
)

const updaterModule: AppModule = createModule({
  metadata: {
    name: 'updater-desktop',
    version: '1.0.0',
    description: 'Desktop-only About page + application auto-updater UI.',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/about',
      element: AboutPage,
      requiresAuth: true,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'Updater',
      store: useUpdaterStore,
    },
  ],
  slots: {
    // User-facing (not admin-gated): About/version + updates.
    settingsUserPages: [
      {
        id: 'about',
        icon: <Info />,
        label: 'About',
        path: 'about',
        // Last user entry — sits after 'general' (10).
        order: 100,
      },
    ],
    // Update card in the left sider footer, just above the user profile
    // (profile = order 100). Renders only when an update is available.
    sidebarFooter: [
      {
        id: 'updater-banner',
        component: UpdateBanner,
        order: 90,
      },
    ],
  },
})

export default updaterModule
