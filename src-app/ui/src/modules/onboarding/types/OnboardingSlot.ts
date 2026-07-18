import type { OnboardingStep } from './onboarding'

export interface OnboardingSlot {
  id: string
  title: string
  description: string
  order: number
  steps: OnboardingStep[]
}

declare module '@ziee/framework/module-system/types' {
  interface Slots {
    onboarding: OnboardingSlot[]
  }
}

export {}
