import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { createStoreProxy, Stores } from '@/core/stores'
import { ApiClient } from '@/api-client'
import '../events/types'

interface OnboardingScreenStore {
  nextEnabled: boolean
  nextLoading: boolean
  nextError: string | null

  setReady: (enabled: boolean) => void
  setNextLoading: (loading: boolean) => void
  setNextError: (error: string | null) => void
  reset: () => void

  completeGuide: (guideId: string) => Promise<void>
  completeStep: (guideId: string, stepId: string) => Promise<void>
}

export const useOnboardingScreenStore = create<OnboardingScreenStore>()(
  subscribeWithSelector(
    immer((set) => ({
      nextEnabled: true,
      nextLoading: false,
      nextError: null,

      setReady: (enabled: boolean) => {
        set(draft => { draft.nextEnabled = enabled })
      },

      setNextLoading: (loading: boolean) => {
        set(draft => { draft.nextLoading = loading })
      },

      setNextError: (error: string | null) => {
        set(draft => { draft.nextError = error })
      },

      reset: () => {
        set(draft => {
          draft.nextEnabled = true
          draft.nextLoading = false
          draft.nextError = null
        })
      },

      completeGuide: async (guideId: string) => {
        const updatedUser = await ApiClient.OnboardingScreen.complete(
          { guide_id: guideId },
          undefined,
        )
        await Stores.EventBus.emit({ type: 'onboarding_screen.user_updated', data: { user: updatedUser } })
      },

      completeStep: async (guideId: string, stepId: string) => {
        const updatedUser = await ApiClient.OnboardingScreen.completeStep(
          { guide_id: guideId, step_id: stepId },
          undefined,
        )
        await Stores.EventBus.emit({ type: 'onboarding_screen.user_updated', data: { user: updatedUser } })
      },
    })),
  ),
)

export const OnboardingScreenStoreProxy = createStoreProxy(useOnboardingScreenStore)
