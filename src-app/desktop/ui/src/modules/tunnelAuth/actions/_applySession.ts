import { Stores } from '@ziee/framework/stores'
import type { AuthResponse } from '@/api-client/types'

/**
 * Hand the whole token pair to the shared Auth store: it captures the body
 * refresh token, records expires_in, and schedules the proactive silent
 * refresh. On refresh failure the shared store clears auth and AuthGuard
 * bounces back to PhoneAuthPage (correct for a remote phone session).
 */
export default function applySession(res: AuthResponse): void {
  Stores.Auth.setAuthFromAutoLogin({
    user: res.user,
    access_token: res.access_token,
    refresh_token: res.refresh_token,
    expires_in: res.expires_in,
  })
}
