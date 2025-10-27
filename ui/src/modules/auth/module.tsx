import { createModule } from '@/core'
import { AuthPage } from './AuthPage'
import { useAuthStore } from './store'

export default createModule({
  metadata: {
    name: 'auth',
    version: '1.0.0',
    description: 'Authentication module with login and registration',
  },
  routes: [
    {
      path: '/auth',
      element: <AuthPage />,
      requiresAuth: false,
      layout: 'none',
    },
  ],
  stores: [
    {
      name: 'Auth',
      store: useAuthStore,
    },
  ],
  initialize: () => {
    console.log('Auth module initialized')
  },
})
