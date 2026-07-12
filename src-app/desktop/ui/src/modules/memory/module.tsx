/**
 * Desktop Memory module — registers a combined route + slot so the
 * single-admin desktop user sees one "Memory" settings entry that
 * stacks both the user-preferences and admin-config sections that
 * core normally splits across `/settings/memory` and
 * `/settings/memory-admin`.
 *
 * Core's two slot entries are filtered out by desktop's SettingsPage
 * HIDDEN_ITEMS list. Core's two routes still exist; URL-typing
 * `/settings/memory` directly would still work, but the menu only
 * exposes the combined entry below.
 */

import { createModule, type AppModule } from '@ziee/ui-core'
import { lazy } from 'react'
import { Book } from 'lucide-react'
import { SettingsLayoutDef } from '@ziee/ui-core/modules/settings/SettingsLayout'

// This module stays a TIER-1 desktop-tree module (NOT a `.desktop.tsx`
// co-location): `module.tsx` files are discovered by `import.meta.glob`, which
// bypasses the `@/` resolver — so a core-tree `module.desktop.tsx` would be
// found by neither `desktop-loader.ts` (globs the desktop tree) nor the core
// `loader.ts` (globs the literal `module.tsx`). It is glob-discovered here by
// `desktop-loader.ts` and registers `memory-desktop`. See DRIFT-1.5.
const MemoryCombinedPage = lazy(() =>
  import('./pages/MemoryCombinedPage').then((m) => ({
    default: m.MemoryCombinedPage,
  })),
)

const memoryDesktopModule: AppModule = createModule({
  metadata: {
    name: 'memory-desktop',
    version: '1.0.0',
    description:
      'Desktop combined Memory settings (user preferences + admin config in one page).',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/memory-combined',
      element: MemoryCombinedPage,
      requiresAuth: true,
      layout: SettingsLayoutDef,
    },
  ],
  slots: {
    settingsUserPages: [
      {
        id: 'memory-desktop',
        icon: <Book />,
        label: 'Memory',
        path: 'memory-combined',
        // Same order as core's user-side Memory entry so the desktop
        // menu position matches what a web user is used to.
        order: 30,
      },
    ],
  },
})

export default memoryDesktopModule
