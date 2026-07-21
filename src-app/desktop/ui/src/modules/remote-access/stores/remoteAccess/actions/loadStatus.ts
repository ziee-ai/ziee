import { ApiClient } from '@/api-client'
import type { RemoteAccessSet, RemoteAccessGet } from '../state'
import type { RemoteAccessStatus } from '../types'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => {
  return async () => {
    set((s) => {
      s.loading = true
      s.error = null
    })
    try {
      const raw = await ApiClient.RemoteAccess.getStatus(undefined, undefined)
      // Normalize generated `string | undefined` → `string | null`.
      const status: RemoteAccessStatus = {
        password_rotated: raw.password_rotated,
        password_auth_enabled: raw.password_auth_enabled,
        auth_token_set: raw.auth_token_set,
        ngrok_domain: raw.ngrok_domain ?? null,
        auto_start_tunnel: raw.auto_start_tunnel,
        tunnel_state: raw.tunnel_state as RemoteAccessStatus['tunnel_state'],
        public_url: raw.public_url ?? null,
        last_error: raw.last_error ?? null,
        started_at: raw.started_at ?? null,
      }
      set((s) => {
        s.status = status
        s.loading = false
      })
      // If connected without a cached magic link, mint one + start rotation.
      if (status.tunnel_state === 'connected' && status.public_url) {
        if (!get().magicLink) await get().rotateMagicLink()
        get().startMagicLinkRotation()
      } else {
        get().stopMagicLinkRotation()
        set((s) => {
          s.magicLink = null
        })
      }
    } catch (e) {
      set((s) => {
        s.loading = false
        s.error = e instanceof Error ? e.message : 'Failed to load status'
      })
    }
  }
}
