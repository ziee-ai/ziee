import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { createStoreProxy } from '@/core/stores'
import { ApiClient } from '@/api-client'
import { useAuthStore } from '@/modules/auth/Auth.store'

interface OnboardingStore {
  // Wizard UI state
  nextEnabled: boolean
  nextLoading: boolean
  nextError: string | null

  // Per-user progress (owned here, not on Stores.Auth.user). `loaded`
  // gates the redirect so it can't mis-fire before the first fetch.
  completedGuideIds: string[]
  completedStepIds: string[]
  loading: boolean
  loaded: boolean

  setReady: (enabled: boolean) => void
  setNextLoading: (loading: boolean) => void
  setNextError: (error: string | null) => void
  reset: () => void

  loadProgress: () => Promise<void>
  completeGuide: (guideId: string) => Promise<void>
  completeStep: (guideId: string, stepId: string) => Promise<void>

  __init__: { __store__: () => void }
  __destroy__: () => void
}

// Module-scope handle to the auth subscription so __destroy__ can detach it.
let unsubscribeAuth: (() => void) | null = null
// Monotonic token so a superseded in-flight load (e.g. after a fast
// user switch) can't overwrite the current user's progress.
let loadToken = 0

export const useOnboardingStore = create<OnboardingStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      nextEnabled: true,
      nextLoading: false,
      nextError: null,

      completedGuideIds: [],
      completedStepIds: [],
      loading: false,
      loaded: false,

      setReady: (enabled: boolean) => {
        set(draft => { draft.nextEnabled = enabled })
      },

      setNextLoading: (loading: boolean) => {
        set(draft => { draft.nextLoading = loading })
      },

      setNextError: (error: string | null) => {
        set(draft => { draft.nextError = error })
      },

      // UI-state reset only (called mid-wizard on step/guide transitions).
      // Does NOT touch progress — see the logout branch in __init__ for that.
      reset: () => {
        set(draft => {
          draft.nextEnabled = true
          draft.nextLoading = false
          draft.nextError = null
        })
      },

      loadProgress: async () => {
        const token = ++loadToken
        set(draft => { draft.loading = true })
        try {
          const progress = await ApiClient.Onboarding.getProgress(undefined, undefined)
          // A newer load (user switched) superseded this one — discard.
          if (token !== loadToken) return
          set(draft => {
            draft.completedGuideIds = progress.completed_guide_ids
            draft.completedStepIds = progress.completed_step_ids
            draft.loaded = true
          })
        } finally {
          if (token === loadToken) set(draft => { draft.loading = false })
        }
      },

      completeGuide: async (guideId: string) => {
        const progress = await ApiClient.Onboarding.complete(
          { guide_id: guideId },
          undefined,
        )
        set(draft => {
          draft.completedGuideIds = progress.completed_guide_ids
          draft.completedStepIds = progress.completed_step_ids
        })
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
      },

      __init__: {
        __store__: () => {
          // Onboarding depends on auth (allowed direction). Subscribe to
          // the USER ID (not isAuthenticated) so a user *switch* — which
          // keeps isAuthenticated=true throughout — still re-fires; a
          // boolean selector would miss admin→user and leave stale
          // progress. Subscribe to the RAW auth store: going through the
          // Stores.Auth proxy in this non-component context triggers
          // useEffect+useStore hooks and corrupts hook ordering (see
          // ProjectFiles.store.ts).
          unsubscribeAuth = useAuthStore.subscribe(
            state => state.user?.id ?? null,
            userId => {
              // Clear the previous identity's progress immediately so
              // nothing stale leaks across login/switch/logout; loaded
              // flips back to false so OnboardingRedirect waits for the
              // fresh fetch.
              set(draft => {
                draft.completedGuideIds = []
                draft.completedStepIds = []
                draft.loaded = false
              })
              if (userId) {
                void get().loadProgress()
              }
            },
            { fireImmediately: true },
          )
        },
      },

      __destroy__: () => {
        unsubscribeAuth?.()
        unsubscribeAuth = null
      },
    })),
  ),
)

export const OnboardingStoreProxy = createStoreProxy(useOnboardingStore)
