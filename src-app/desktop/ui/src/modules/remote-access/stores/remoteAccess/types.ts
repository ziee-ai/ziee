/** Re-exported types from the former monolithic store — consumers that import
 *  from the old path still get the same types via the barrel. */

export type TunnelStateKind = 'idle' | 'starting' | 'connected' | 'error'

export interface RemoteAccessStatus {
  password_rotated: boolean
  password_auth_enabled: boolean
  auth_token_set: boolean
  ngrok_domain: string | null
  auto_start_tunnel: boolean
  tunnel_state: TunnelStateKind
  public_url: string | null
  last_error: string | null
  started_at: string | null
}

export interface MagicLink {
  /** Plaintext token returned by /api/auth/magic-link/issue. */
  token: string
  /** Pre-formatted URL: https://<public_url>/auth/magic/<token>. */
  url: string
  /** Expiry timestamp from the server. */
  expires_at: string
  /** When the page issued this token (for the countdown). */
  issued_at: string
}
