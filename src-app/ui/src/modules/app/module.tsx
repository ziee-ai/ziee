import { createModule, Stores } from '@/core'
import { useAppStore } from '@/modules/app/App.store'
import { useAppModeStore } from '@/modules/app/AppMode.store'
import { BlankLayout } from '@/modules/layouts/blank'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useEffect } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'

const SetupPage = lazyWithPreload(() => import('./SetupPage'))

/**
 * SetupRedirect — routes to /setup when the app needs first-time admin
 * setup. Mounted inside <BrowserRouter> via the `routerEffects` slot so
 * it can use `useNavigate` but renders nothing.
 */
function SetupRedirect() {
  const needsSetup = Stores.App.needsSetup
  const navigate = useNavigate()
  const location = useLocation()

  useEffect(() => {
    if (needsSetup && location.pathname !== '/setup') {
      navigate('/setup', { replace: true })
    }
  }, [needsSetup, navigate, location.pathname])

  return null
}

export default createModule({
  metadata: {
    name: 'app',
    version: '1.0.0',
    description: 'Application-level module',
  },
  routes: [
    {
      path: '/setup',
      element: SetupPage,
      requiresAuth: false,
      layout: BlankLayout,
    },
  ],
  stores: [
    {
      name: 'App',
      store: useAppStore,
    },
    {
      // Portable multi-user / single-admin flag. Default `true` (web
      // build); the desktop bootstrap flips it to `false`. See
      // AppMode.store.ts.
      name: 'AppMode',
      store: useAppModeStore,
    },
  ],
  slots: {
    routerEffects: [
      {
        id: 'app-setup-redirect',
        component: SetupRedirect,
      },
    ],
  },
  initialize: async () => {
    // Check setup status on app initialization
    await Stores.App.checkSetupStatus()
    if (Stores.App.__state.needsSetup) {
      console.log('Application needs setup')
    } else {
      console.log('Application is already set up')
    }
    console.log('App module initialized')
  },
})
