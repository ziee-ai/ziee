import { createModule } from '@/core'
import { useAuthStore } from '@/modules/auth/Auth.store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const AuthPage = lazyWithPreload(() =>
  import('./AuthPage').then(m => ({ default: m.AuthPage })),
)

export default createModule({
  metadata: {
    name: 'auth',
    version: '1.0.0',
    description: 'Authentication module with login and registration',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/login',
      element: AuthPage,
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
