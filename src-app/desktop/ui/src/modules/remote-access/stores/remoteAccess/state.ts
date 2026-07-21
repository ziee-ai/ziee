import type { StoreSet } from '@ziee/framework/store-kit'

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

/** Rotation interval: 4 min, comfortably under the 5-min server-side TTL. */
export const ROTATION_INTERVAL_MS = 4 * 60 * 1000

export const remoteAccessState = {
  status: null as RemoteAccessStatus | null,
  loading: false,
  saving: false,
  error: null as string | null,
  /** Current magic-link token + URL. Null when tunnel is not connected. */
  magicLink: null as MagicLink | null,
  /** Rotation timer so we can clear it on unmount / tunnel stop. */
  rotationTimer: null as ReturnType<typeof setInterval> | null,
}

/** The raw data shape (the `state:` object). */
export type RemoteAccessData = typeof remoteAccessState

/**
 * Full store shape (data + actions). Used by the `RegisteredStores` augmentation
 * AND by actions that call SIBLING actions via `get().loadStatus()` etc. — those
 * resolve to the store's (lazy) dispatchers at call time, so cross-action calls
 * need no factory import.
 */
export interface RemoteAccessState extends RemoteAccessData {
  loadStatus: () => Promise<void>
  saveAuthToken: (token: string) => Promise<void>
  saveDomain: (domain: string | null) => Promise<void>
  saveAutoStart: (enabled: boolean) => Promise<void>
  setPasswordAuthEnabled: (enabled: boolean) => Promise<void>
  setAdminPassword: (newPassword: string) => Promise<void>
  startTunnel: () => Promise<void>
  stopTunnel: () => Promise<void>
  rotateMagicLink: () => Promise<void>
  startMagicLinkRotation: () => void
  stopMagicLinkRotation: () => void
}

export type RemoteAccessSet = StoreSet<RemoteAccessData>
export type RemoteAccessGet = () => RemoteAccessState
