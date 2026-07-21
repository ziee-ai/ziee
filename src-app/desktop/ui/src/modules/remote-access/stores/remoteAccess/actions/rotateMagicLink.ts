import { ApiClient } from '@/api-client'
import type { RemoteAccessGet, RemoteAccessSet } from '../state'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => async () => {
  const status = get().status
  if (!status || status.tunnel_state !== 'connected' || !status.public_url) return
  try {
    const issued = await ApiClient.Auth.magicLinkIssue(undefined, undefined)
    const trimmed = status.public_url.replace(/\/$/, '')
    const url = `${trimmed}/auth/magic/${issued.token}`
    set(s => {
      s.magicLink = {
        token: issued.token,
        url,
        expires_at: issued.expires_at,
        issued_at: new Date().toISOString(),
      }
    })
  } catch (e) {
    // Non-fatal: the existing magic link is valid until it expires.
    console.warn('[RemoteAccess] magic-link rotation failed:', e)
  }
}
