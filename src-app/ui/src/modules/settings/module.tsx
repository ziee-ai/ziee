import { createModule } from '@/core'
import { SettingOutlined } from '@ant-design/icons'
import { SettingsLayoutDef } from './SettingsLayout'

// Bare /settings renders no content of its own — SettingsPage (provided by the
// SettingsLayout layout) redirects to the first permitted section. Routing it
// through SettingsLayoutDef — the SAME layout every /settings/* sub-page uses —
// keeps one settings AppLayout mounted across that redirect.
//
// The previous `element: SettingsLayout, layout: AppLayoutDef` rendered
// AppLayout TWICE (once as the route layout, once inside the lazy
// SettingsLayout) and put /settings in a different layout group than its
// sub-pages. So opening Settings flashed the app's AppLayout (with the chat
// sider) before swapping to the settings layout.
const SettingsIndex = () => null

export default createModule({
  metadata: {
    name: 'settings',
    version: '1.0.0',
    description: 'Settings module for user preferences',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings',
      element: <SettingsIndex />,
      requiresAuth: true,
      layout: SettingsLayoutDef,
    },
  ],
  slots: {
    sidebarTools: [
      {
        id: 'settings',
        icon: <SettingOutlined />,
        label: 'Settings',
        path: '/settings',
        order: 100,
      },
    ],
  },
  initialize: () => {},
})
