import { createModule, Stores } from '@/core'
import { useAppStore, checkSetupStatus } from './store'
import { BlankLayout } from '@/components/Layout/BlankLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const SetupPage = lazyWithPreload(() => import('./SetupPage'))

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
  ],
  initialize: async () => {
    // Check setup status on app initialization
    await checkSetupStatus()
    if (Stores.App.__state.needsSetup) {
      console.log('Application needs setup')
      if (window.location.pathname !== '/setup') window.location.href = '/setup'
    } else {
      console.log('Application is already set up')
    }
    console.log('App module initialized')
  },
})
