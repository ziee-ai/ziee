import type React from 'react'

export interface OnboardingStepProps {
  /**
   * Register an async action to run when Next is clicked (before advancing).
   * Pass null to clear. Function reads from stores at call time — no stale closure risk.
   */
  registerBeforeNext: (fn: (() => Promise<void>) | null) => void
}

export interface OnboardingStep {
  id: string
  title: string
  component: React.ComponentType<OnboardingStepProps>
  /**
   * If true (default), Next starts enabled — user can advance without doing anything.
   * If false, Next starts disabled until the step calls OnboardingStore.setReady(true).
   */
  skippable?: boolean
}

export interface Onboarding {
  id: string
  title: string
  description: string
  steps: OnboardingStep[]
  /** Used for ordering in the onboarding list. Lower = first. */
  order?: number
}
