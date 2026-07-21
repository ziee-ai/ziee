import { ApiClient } from '@/api-client'
import type { ChangePasswordRequest } from '@/api-client/types'
import type { ProfileGet, ProfileSet } from '../state'

export default (set: ProfileSet, _get: ProfileGet) => {
  return async (patch: ChangePasswordRequest) => {
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
  }
}
