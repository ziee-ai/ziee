import { ApiClient } from '@/api-client'
import { Stores } from '@ziee/framework/stores'
import type { UpdateProfileRequest } from '@/api-client/types'
import { emitProfileUpdated } from '@/modules/profile/events'
import type { ProfileGet, ProfileSet } from '../state'

export default (set: ProfileSet, _get: ProfileGet) => {
  return async (patch: UpdateProfileRequest) => {
    // Update the current user's own profile, then refresh Auth so the sidebar
    // stays in sync. Throws on failure (the page surfaces the message).
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
  }
}
