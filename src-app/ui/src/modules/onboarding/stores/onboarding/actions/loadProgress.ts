import { ApiClient } from '@/api-client'
import type { OnboardingGet, OnboardingSet } from '../state'

// Monotonic token so a superseded in-flight load (e.g. after a fast user
// switch) can't overwrite the current user's progress.
let loadToken = 0

export default (set: OnboardingSet, _get: OnboardingGet) =>
  async () => {
    const token = ++loadToken
    set(draft => {
      draft.loading = true
    })
    try {
      const progress = await ApiClient.Onboarding.getProgress(undefined, undefined)
      // A newer load (user switched) superseded this one — discard.
      if (token !== loadToken) return
      set(draft => {
        draft.completedGuideIds = progress.completed_guide_ids
        draft.completedStepIds = progress.completed_step_ids
        draft.loaded = true
      })
    } catch (error) {
      // Progress is a non-blocking enhancement: the guides themselves come
      // from module slots, so the page still renders with defaults (start at
      // the first step). Swallow the failure — an uncaught rejection here
      // would otherwise bubble to the ErrorBoundary and blank the wizard.
      if (token === loadToken) {
        console.error('Failed to load onboarding progress:', error)
      }
    } finally {
      if (token === loadToken)
        set(draft => {
          draft.loading = false
        })
    }
  }
