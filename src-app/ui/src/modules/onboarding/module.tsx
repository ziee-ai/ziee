import { createModule } from '@/core'
import './events/types'
import { BookOutlined } from '@ant-design/icons'
import { BlankLayout } from '@/modules/layouts/blank'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useOnboardingStore } from './stores/Onboarding.store'
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
  dependencies: ['router'],
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
        icon: <BookOutlined />,
        label: 'Onboarding',
        path: '/onboarding',
        order: 90,
      },
    ],
  },
})
