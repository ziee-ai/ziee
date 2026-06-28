import { createModule } from '@/core'
import { Book } from 'lucide-react'
import { BlankLayout } from '@/modules/layouts/blank'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useOnboardingStore } from './stores/Onboarding.store'
import { OnboardingRedirect } from './OnboardingRedirect'
import './types/OnboardingSlot'
import './types'

const OnboardingPage = lazyWithPreload(
  () => import('./OnboardingPage'),
)

export default createModule({
  metadata: {
    name: 'onboarding',
    version: '1.0.0',
    description: 'Onboarding guides',
  },
  dependencies: ['router', 'auth'],
  stores: [
    { name: 'Onboarding', store: useOnboardingStore },
  ],
  routes: [
    {
      path: '/onboarding',
      element: OnboardingPage,
      requiresAuth: true,
      layout: BlankLayout,
    },
  ],
  slots: {
    sidebarTools: [
      {
        id: 'onboarding',
        icon: <Book />,
        label: 'Onboarding',
        path: '/onboarding',
        order: 90,
      },
    ],
    // Self-owned post-auth redirect. Mounted inside <BrowserRouter>
    // by RouterComponent — uses useNavigate/useLocation, runs in a
    // useEffect, returns null. Auth + router stay decoupled from
    // onboarding.
    routerEffects: [
      {
        id: 'onboarding-redirect',
        component: OnboardingRedirect,
      },
    ],
  },
})
