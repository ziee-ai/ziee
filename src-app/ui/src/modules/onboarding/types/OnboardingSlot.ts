import type { OnboardingStep } from './onboarding'

export interface OnboardingSlot {
  id: string
  title: string
  description: string
  order: number
  steps: OnboardingStep[]
}

declare module '@/core/module-system/types' {
  interface Slots {
    onboarding: OnboardingSlot[]
    routerEffects: Array<{ id: string; component: React.ComponentType }>
  }
}

export {}
