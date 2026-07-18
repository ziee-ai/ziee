import { ApiClient } from '@/api-client'
import { Stores } from '@ziee/framework/stores'
import type {
  ChangePasswordRequest,
  UpdateProfileRequest,
} from '@/api-client/types'
import { emitProfileUpdated } from '@/modules/profile/events'
import { defineStore } from '@ziee/framework/store-kit'

export const Profile = defineStore('Profile', {
  immer: true,
  state: { savingProfile: false, savingPassword: false },
  actions: set => ({
    // Update the current user's own profile, then refresh Auth so the sidebar
    // stays in sync. Throws on failure (the page surfaces the message).
    updateProfile: async (patch: UpdateProfileRequest) => {
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
    changePassword: async (patch: ChangePasswordRequest) => {
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
  }),
})

export const useProfileStore = Profile.store
