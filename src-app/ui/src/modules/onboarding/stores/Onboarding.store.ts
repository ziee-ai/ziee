import { ApiClient } from '@/api-client'
import type { BaseEvent } from '@/core/events'
import { defineStore } from '@/core/store-kit'
import { createStoreProxy, Stores } from '@/core/stores'
import { useAuthStore } from '@/modules/auth/Auth.store'

// Monotonic token so a superseded in-flight load (e.g. after a fast user
// switch) can't overwrite the current user's progress.
let loadToken = 0

export const Onboarding = defineStore('Onboarding', {
  immer: true,
  state: {
    // Wizard UI state
    nextEnabled: true,
    nextLoading: false,
    nextError: null as string | null,
    // Per-user progress (owned here, not on Stores.Auth.user). `loaded` gates
    // the redirect so it can't mis-fire before the first fetch.
    completedGuideIds: [] as string[],
    completedStepIds: [] as string[],
    loading: false,
    loaded: false,
  },
  actions: set => ({
    setReady: (enabled: boolean) => {
      set(draft => {
        draft.nextEnabled = enabled
      })
    },
    setNextLoading: (loading: boolean) => {
      set(draft => {
        draft.nextLoading = loading
      })
    },
    setNextError: (error: string | null) => {
      set(draft => {
        draft.nextError = error
      })
    },
    // UI-state reset only (mid-wizard step/guide transitions). Does NOT touch
    // progress — see the auth watch below for that.
    reset: () => {
      set(draft => {
        draft.nextEnabled = true
        draft.nextLoading = false
        draft.nextError = null
      })
    },
    loadProgress: async () => {
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
    },
    completeGuide: async (guideId: string) => {
      const progress = await ApiClient.Onboarding.complete({ guide_id: guideId }, undefined)
      set(draft => {
        draft.completedGuideIds = progress.completed_guide_ids
        draft.completedStepIds = progress.completed_step_ids
      })
      await Stores.EventBus.emit({ type: 'onboarding.guide_completed', data: { guideId } })
    },
    completeStep: async (guideId: string, stepId: string) => {
      const progress = await ApiClient.Onboarding.completeStep(
        { guide_id: guideId, step_id: stepId },
        undefined,
      )
      set(draft => {
        draft.completedGuideIds = progress.completed_guide_ids
        draft.completedStepIds = progress.completed_step_ids
      })
      await Stores.EventBus.emit({
        type: 'onboarding.step_completed',
        data: { guideId, stepId },
      })
    },
  }),
  init: ({ watch, on, set, actions }) => {
    // Onboarding depends on auth (allowed direction). Watch the USER ID (not
    // isAuthenticated) so a user *switch* — which keeps isAuthenticated=true —
    // still re-fires. Watch the RAW auth store: going through the Stores.Auth
    // proxy in this non-component context corrupts hook ordering.
    watch(
      useAuthStore,
      state => state.user?.id ?? null,
      userId => {
        // Clear the previous identity's progress immediately so nothing stale
        // leaks across login/switch/logout; loaded flips to false so the
        // redirect waits for the fresh fetch.
        set(draft => {
          draft.completedGuideIds = []
          draft.completedStepIds = []
          draft.loaded = false
        })
        if (userId) void actions.loadProgress()
      },
      { fireImmediately: true },
    )
    // Cross-device sync: progress advancing on another device (or a missed
    // event across a dropped stream) refetches here. No permission self-gate —
    // `GET /api/onboarding/progress` is JwtAuth-only (no perm), and both
    // triggers only fire on an authenticated SSE stream, so there's no 403 to
    // guard against. Only refetch if we already have an identity loaded.
    const reload = () => {
      if (useAuthStore.getState().user?.id) void actions.loadProgress()
    }
    on('sync:onboarding', reload)
    on('sync:reconnect', reload)
  },
})

export const useOnboardingStore = Onboarding.store

// Events
export interface OnboardingGuideCompletedEvent extends BaseEvent {
  type: 'onboarding.guide_completed'
  data: { guideId: string }
}

export interface OnboardingStepCompletedEvent extends BaseEvent {
  type: 'onboarding.step_completed'
  data: { guideId: string; stepId: string }
}

declare module '@/core/events' {
  interface AppEvents {
    'onboarding.guide_completed': OnboardingGuideCompletedEvent
    'onboarding.step_completed': OnboardingStepCompletedEvent
  }
}

export const OnboardingStoreProxy = createStoreProxy(useOnboardingStore)
