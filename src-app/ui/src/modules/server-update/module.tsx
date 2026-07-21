import { createModule } from '@ziee/framework'
import { MdInfoOutline } from 'react-icons/md'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { useServerUpdateStore } from '@/modules/server-update/stores/serverUpdate'
import { ServerUpdateBanner } from '@/modules/server-update/ServerUpdateBanner'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { Permissions } from '@/api-client/permissions'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import '@/modules/layouts/app-layout/types' // Register the appBanners slot type

const AboutSettings = lazyWithPreload(() => import('./AboutSettings'))

export default createModule({
  metadata: {
    name: 'server-update',
    version: '1.0.0',
    description: 'Server version + update notification (admin).',
  },
  routes: [
    {
      path: '/settings/about',
      element: AboutSettings,
      requiresAuth: true,
      permission: Permissions.ServerUpdateRead,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'ServerUpdate',
      store: useServerUpdateStore,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'about',
        icon: <MdInfoOutline />,
        label: 'About',
        path: 'about',
        order: 100,
        permission: Permissions.ServerUpdateRead,
      },
    ],
    appBanners: [
      {
        id: 'server-update-banner',
        component: ServerUpdateBanner,
        order: 10,
      },
    ],
  },
})
