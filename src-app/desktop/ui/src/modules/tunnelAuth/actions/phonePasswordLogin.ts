import { ApiClient } from '@/api-client'
import type { TunnelAuthGet, TunnelAuthSet } from '../state'
import applySession from './_applySession'

export default (set: TunnelAuthSet, get: TunnelAuthGet) =>
  async (password: string) => {
    if (get().submittingPassword) return
    set({ submittingPassword: true, passwordError: null })
    try {
      const res = await ApiClient.Auth.loginPasswordOnly({ password }, undefined)
      applySession(res)
      set({ submittingPassword: false })
    } catch (e) {
      set({
        submittingPassword: false,
        passwordError: e instanceof Error ? e.message : 'Login failed',
      })
      throw e
    }
  }
