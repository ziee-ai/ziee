import { ApiClient } from '@/api-client'
import type { TunnelAuthGet, TunnelAuthSet } from '../state'
import applySession from './_applySession'

export default (set: TunnelAuthSet, get: TunnelAuthGet) =>
  async (token: string) => {
    // Dedupe StrictMode double-mount + browser-refresh: no-op if already
    // exchanging THIS token.
    if (get().exchangingToken === token) return
    set({ exchangingToken: token, exchangeError: null })
    try {
      const res = await ApiClient.Auth.magicLinkExchange({ token }, undefined)
      applySession(res)
      set({ exchangingToken: null })
    } catch (e) {
      set({
        exchangingToken: null,
        exchangeError: e instanceof Error ? e.message : 'Exchange failed',
      })
      throw e
    }
  }
