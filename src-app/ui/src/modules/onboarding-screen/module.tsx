import { createModule } from '@/core'
import './events/types'
import { BookOutlined } from '@ant-design/icons'
import { BlankLayout } from '@/modules/layouts/blank'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useOnboardingScreenStore } from './stores/OnboardingScreen.store'
import './types/OnboardingSlot'
import './types'

const OnboardingScreenPage = lazyWithPreload(
  () => import('./OnboardingScreenPage'),
)

export default createModule({
  metadata: {
    name: 'onboarding-screen',
    version: '1.0.0',
    description: 'Onboarding guides',
  },
  dependencies: ['router'],
  stores: [
    { name: 'OnboardingScreen', store: useOnboardingScreenStore },
  ],
  routes: [
    {
      path: '/onboarding-screen',
      element: OnboardingScreenPage,
      requiresAuth: true,
      layout: BlankLayout,
    },
  ],
  slots: {
    sidebarTools: [
      {
        id: 'onboarding-screen',
        icon: <BookOutlined />,
        label: 'Onboarding',
        path: '/onboarding-screen',
        order: 90,
      },
    ],
  },
})
