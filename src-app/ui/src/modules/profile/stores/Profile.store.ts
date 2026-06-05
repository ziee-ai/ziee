import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import type {
  ChangePasswordRequest,
  UpdateProfileRequest,
} from '@/api-client/types'
import { emitProfileUpdated } from '@/modules/profile/events'

interface ProfileStore {
  savingProfile: boolean
  savingPassword: boolean

  // Update the current user's own profile (username + display_name),
  // then refresh the Auth store so the sidebar widget + password
  // section stay in sync without a reload. Throws on failure — the page
  // surfaces the API's error message via `message.error`.
  updateProfile: (patch: UpdateProfileRequest) => Promise<void>
  // Change the current user's own password. Throws on failure.
  changePassword: (patch: ChangePasswordRequest) => Promise<void>
}

export const useProfileStore = create<ProfileStore>()(
  subscribeWithSelector(
    immer(set => ({
      savingProfile: false,
      savingPassword: false,

      updateProfile: async patch => {
        set(s => {
          s.savingProfile = true
        })
        try {
          const user = await ApiClient.Auth.updateProfile(patch)
          await Stores.Auth.refreshCurrentUser()
          try {
            await emitProfileUpdated(user)
          } catch (eventError) {
            console.error('Failed to emit profile.updated event:', eventError)
          }
        } finally {
          set(s => {
            s.savingProfile = false
          })
        }
      },

      changePassword: async patch => {
        set(s => {
          s.savingPassword = true
        })
        try {
          await ApiClient.Auth.changePassword(patch)
        } finally {
          set(s => {
            s.savingPassword = false
          })
        }
      },
    })),
  ),
)
