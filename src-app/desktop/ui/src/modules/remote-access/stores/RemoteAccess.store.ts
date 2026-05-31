/**
 * Remote Access Zustand store.
 *
 * Wraps the typed `ApiClient.RemoteAccess` + `ApiClient.Auth` methods
 * (generated from the backend's aide schema) so the UI only deals
 * with `Stores.RemoteAccess.startTunnel()` etc., never raw fetches.
 *
 * Magic-link token rotation lives here too: the page polls
 * `issueMagicLink()` every 4 minutes (1 min before the 5-min token
 * expires) and stores the resulting plaintext URL for the QR + Copy
 * affordances.
 */

import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import { type StoreProxy } from '@/core/stores'
import { emitRemoteAccessStatusChanged } from '@ziee/desktop/modules/remote-access/events/remote-access-events'

// =====================================================
// Public types (mirror the backend response shapes)
// =====================================================

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

// =====================================================
// Store state
// =====================================================

interface RemoteAccessState {
  status: RemoteAccessStatus | null
  loading: boolean
  saving: boolean
  error: string | null

  /** Current magic-link token + URL. Null when tunnel is not
   * connected. The page rotates this every 4 minutes. */
  magicLink: MagicLink | null
  /** Track the rotation timer so we can clear it on unmount /
   * tunnel stop. */
  rotationTimer: ReturnType<typeof setInterval> | null

  __init__: {
    status: () => Promise<void>
  }
  __destroy__: () => void

  // Status / settings actions
  loadStatus: () => Promise<void>
  saveAuthToken: (token: string) => Promise<void>
  saveDomain: (domain: string | null) => Promise<void>
  saveAutoStart: (enabled: boolean) => Promise<void>
  setPasswordAuthEnabled: (enabled: boolean) => Promise<void>
  /** Rotate the admin password (bypass-current-password endpoint, gated
   *  by the localhost-Host middleware). Used by the inline form when
   *  enabling password auth on a still-bootstrap admin. */
  setAdminPassword: (newPassword: string) => Promise<void>

  // Tunnel actions
  startTunnel: () => Promise<void>
  stopTunnel: () => Promise<void>

  // Magic-link actions
  rotateMagicLink: () => Promise<void>
  startMagicLinkRotation: () => void
  stopMagicLinkRotation: () => void
}

declare module '@/core/stores' {
  interface RegisteredStores {
    RemoteAccess: StoreProxy<RemoteAccessState>
  }
}

// Rotation interval: 4 min, comfortably under the 5-min server-side
// TTL so a phone scanning at any time gets a working link.
const ROTATION_INTERVAL_MS = 4 * 60 * 1000

// Direct ApiClient usage — the generated types include the full
// RemoteAccess + Auth.magicLink* surfaces; no `as unknown as` cast
// needed. (The cast lived here while the openapi types were still
// out of sync; that's resolved.)
const remoteAccessClient = ApiClient.RemoteAccess
const authClient = ApiClient.Auth

export const useRemoteAccessStore = create<RemoteAccessState>()(
  subscribeWithSelector(
    immer((set, get) => ({
      status: null,
      loading: false,
      saving: false,
      error: null,
      magicLink: null,
      rotationTimer: null,

      __init__: {
        status: async () => {
          // Eager-load so the settings page renders with real data
          // on first mount.
          await get().loadStatus()
        },
      },

      __destroy__: () => {
        get().stopMagicLinkRotation()
      },

      loadStatus: async () => {
        set((s) => {
          s.loading = true
          s.error = null
        })
        try {
          const raw = await remoteAccessClient.getStatus(undefined, undefined)
          // Normalize generated `string | undefined` → `string | null`
          // at the read boundary so the local store type can keep
          // using `null` semantics consistently.
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
          // If the tunnel is connected and we don't have a magic
          // link cached yet, mint one + start rotation. Idempotent
          // — startMagicLinkRotation guards against double-init.
          if (status.tunnel_state === 'connected' && status.public_url) {
            if (!get().magicLink) {
              await get().rotateMagicLink()
            }
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
      },

      saveAuthToken: async (token: string) => {
        await mutate(set, get, async () => {
          await remoteAccessClient.updateSettings(
            { ngrok_auth_token: token },
            undefined,
          )
          await get().loadStatus()
          emitRemoteAccessStatusChanged('settings')
        })
      },

      saveDomain: async (domain: string | null) => {
        await mutate(set, get, async () => {
          // Generated request type uses `string | undefined`; null
          // means "clear it" semantically. The backend's deserialize
          // treats both `null` (explicit clear) and "" the same;
          // pass empty string to disambiguate from "missing field".
          await remoteAccessClient.updateSettings(
            { ngrok_domain: domain ?? '' },
            undefined,
          )
          await get().loadStatus()
          emitRemoteAccessStatusChanged('settings')
        })
      },

      saveAutoStart: async (enabled: boolean) => {
        await mutate(set, get, async () => {
          await remoteAccessClient.updateSettings(
            { auto_start_tunnel: enabled },
            undefined,
          )
          await get().loadStatus()
          emitRemoteAccessStatusChanged('settings')
        })
      },

      setPasswordAuthEnabled: async (enabled: boolean) => {
        await mutate(set, get, async () => {
          await remoteAccessClient.updateSettings(
            { password_auth_enabled: enabled },
            undefined,
          )
          await get().loadStatus()
          emitRemoteAccessStatusChanged('settings')
          // No local auth-config refresh needed — the desktop admin
          // doesn't render the phone login surface; phones fetch
          // /api/auth/config on mount when PhoneAuthPage loads.
        })
      },

      setAdminPassword: async (newPassword: string) => {
        await mutate(set, get, async () => {
          await remoteAccessClient.setAdminPassword(
            { new_password: newPassword },
            undefined,
          )
          // The PUT toggles `password_changed_at`; reload so the
          // status reflects `password_rotated: true`.
          await get().loadStatus()
          emitRemoteAccessStatusChanged('settings')
        })
      },

      startTunnel: async () => {
        await mutate(set, get, async () => {
          await remoteAccessClient.startTunnel(undefined, undefined)
          await get().loadStatus()
          emitRemoteAccessStatusChanged('tunnel')
        })
      },

      stopTunnel: async () => {
        await mutate(set, get, async () => {
          await remoteAccessClient.stopTunnel(undefined, undefined)
          get().stopMagicLinkRotation()
          set((s) => {
            s.magicLink = null
          })
          await get().loadStatus()
          emitRemoteAccessStatusChanged('tunnel')
        })
      },

      rotateMagicLink: async () => {
        const status = get().status
        if (!status || status.tunnel_state !== 'connected' || !status.public_url) {
          return
        }
        try {
          const issued = await authClient.magicLinkIssue(undefined, undefined)
          // Construct the user-visible URL. The public_url already has
          // the scheme + host (e.g. https://my-app.ngrok.app); append
          // the SPA route.
          const trimmed = status.public_url.replace(/\/$/, '')
          const url = `${trimmed}/auth/magic/${issued.token}`
          set((s) => {
            s.magicLink = {
              token: issued.token,
              url,
              expires_at: issued.expires_at,
              issued_at: new Date().toISOString(),
            }
          })
        } catch (e) {
          // Don't surface as a fatal error; the existing magic link
          // is still valid until it expires.
          console.warn('[RemoteAccess] magic-link rotation failed:', e)
        }
      },

      startMagicLinkRotation: () => {
        const existing = get().rotationTimer
        if (existing) return
        const timer = setInterval(() => {
          // Skip the tick when the tab is hidden — the QR isn't on
          // screen, so we don't need to keep minting fresh tokens
          // (each rotation is a DB write + new audit row). The next
          // visible tick will pick up.
          if (
            typeof document !== 'undefined' &&
            document.visibilityState === 'hidden'
          ) {
            return
          }
          void get().rotateMagicLink()
        }, ROTATION_INTERVAL_MS)
        set((s) => {
          s.rotationTimer = timer
        })
      },

      stopMagicLinkRotation: () => {
        const timer = get().rotationTimer
        if (timer) {
          clearInterval(timer)
          set((s) => {
            s.rotationTimer = null
          })
        }
      },
    })),
  ),
)

// Helper: bracket every mutation with saving=true/false + error
// capture so handlers stay terse.
async function mutate(
  set: (recipe: (s: RemoteAccessState) => void) => void,
  _get: () => RemoteAccessState,
  body: () => Promise<void>,
) {
  set((s) => {
    s.saving = true
    s.error = null
  })
  try {
    await body()
  } catch (e) {
    set((s) => {
      s.error = e instanceof Error ? e.message : 'Failed'
    })
    throw e
  } finally {
    set((s) => {
      s.saving = false
    })
  }
}
