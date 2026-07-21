import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { onboardingState, type OnboardingState } from './state'
import { useAuthStore } from '@/modules/auth/Auth.store'
import type { Actions } from './actions.gen'

const OnboardingDef = defineStore<OnboardingState, Actions>('Onboarding', {
  immer: true,
  state: onboardingState,
  actions: import.meta.glob('./actions/*.ts'),
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

export const Onboarding = registerLazyStore(OnboardingDef)
export const useOnboardingStore = OnboardingDef.store
