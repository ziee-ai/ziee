/**
 * Remote Access store. Wraps the typed `ApiClient.RemoteAccess` + `ApiClient.Auth`
 * methods so the UI only deals with `Stores.RemoteAccess.startTunnel()` etc.
 * Magic-link token rotation lives here too: the page rotates `issueMagicLink()`
 * every 4 minutes (1 min before the 5-min token expires).
 */

import { ApiClient } from '@/api-client'
import { defineStore } from '@/core/store-kit'
import { type StoreProxy } from '@/core/stores'
import { emitRemoteAccessStatusChanged } from '@ziee/desktop/modules/remote-access/events/remote-access-events'

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

interface RemoteAccessState {
  status: RemoteAccessStatus | null
  loading: boolean
  saving: boolean
  error: string | null
  /** Current magic-link token + URL. Null when tunnel is not connected. */
  magicLink: MagicLink | null
  /** Rotation timer so we can clear it on unmount / tunnel stop. */
  rotationTimer: ReturnType<typeof setInterval> | null
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

declare module '@/core/stores' {
  interface RegisteredStores {
    RemoteAccess: StoreProxy<RemoteAccessState>
  }
}

// Rotation interval: 4 min, comfortably under the 5-min server-side TTL.
const ROTATION_INTERVAL_MS = 4 * 60 * 1000

const remoteAccessClient = ApiClient.RemoteAccess
const authClient = ApiClient.Auth

// Helper: bracket every mutation with saving=true/false + error capture.
async function mutate(
  // Loosely typed `set` — store-kit hands actions a State-only setter; mutate
  // only touches `saving`/`error`, so a structural draft type is enough.
  set: (recipe: (s: { saving: boolean; error: string | null }) => void) => void,
  body: () => Promise<void>,
) {
  set(s => {
    s.saving = true
    s.error = null
  })
  try {
    await body()
  } catch (e) {
    set(s => {
      s.error = e instanceof Error ? e.message : 'Failed'
    })
    throw e
  } finally {
    set(s => {
      s.saving = false
    })
  }
}

export const RemoteAccess = defineStore('RemoteAccess', {
  immer: true,
  state: {
    status: null as RemoteAccessStatus | null,
    loading: false,
    saving: false,
    error: null as string | null,
    magicLink: null as MagicLink | null,
    rotationTimer: null as ReturnType<typeof setInterval> | null,
  },
  actions: (set, getRaw) => {
    const get = getRaw as () => RemoteAccessState
    return {
      loadStatus: async () => {
        set(s => {
          s.loading = true
          s.error = null
        })
        try {
          const raw = await remoteAccessClient.getStatus(undefined, undefined)
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
          set(s => {
            s.status = status
            s.loading = false
          })
          // If connected without a cached magic link, mint one + start rotation.
          if (status.tunnel_state === 'connected' && status.public_url) {
            if (!get().magicLink) await get().rotateMagicLink()
            get().startMagicLinkRotation()
          } else {
            get().stopMagicLinkRotation()
            set(s => {
              s.magicLink = null
            })
          }
        } catch (e) {
          set(s => {
            s.loading = false
            s.error = e instanceof Error ? e.message : 'Failed to load status'
          })
        }
      },
      saveAuthToken: async (token: string) => {
        await mutate(set, async () => {
          await remoteAccessClient.updateSettings({ ngrok_auth_token: token }, undefined)
          await get().loadStatus()
          emitRemoteAccessStatusChanged('settings')
        })
      },
      saveDomain: async (domain: string | null) => {
        await mutate(set, async () => {
          // null means "clear it"; pass empty string to disambiguate from missing.
          await remoteAccessClient.updateSettings({ ngrok_domain: domain ?? '' }, undefined)
          await get().loadStatus()
          emitRemoteAccessStatusChanged('settings')
        })
      },
      saveAutoStart: async (enabled: boolean) => {
        await mutate(set, async () => {
          await remoteAccessClient.updateSettings({ auto_start_tunnel: enabled }, undefined)
          await get().loadStatus()
          emitRemoteAccessStatusChanged('settings')
        })
      },
      setPasswordAuthEnabled: async (enabled: boolean) => {
        await mutate(set, async () => {
          await remoteAccessClient.updateSettings({ password_auth_enabled: enabled }, undefined)
          await get().loadStatus()
          emitRemoteAccessStatusChanged('settings')
        })
      },
      setAdminPassword: async (newPassword: string) => {
        await mutate(set, async () => {
          await remoteAccessClient.setAdminPassword({ new_password: newPassword }, undefined)
          // The PUT toggles `password_changed_at`; reload so status reflects it.
          await get().loadStatus()
          emitRemoteAccessStatusChanged('settings')
        })
      },
      startTunnel: async () => {
        await mutate(set, async () => {
          await remoteAccessClient.startTunnel(undefined, undefined)
          await get().loadStatus()
          emitRemoteAccessStatusChanged('tunnel')
        })
      },
      stopTunnel: async () => {
        await mutate(set, async () => {
          await remoteAccessClient.stopTunnel(undefined, undefined)
          get().stopMagicLinkRotation()
          set(s => {
            s.magicLink = null
          })
          await get().loadStatus()
          emitRemoteAccessStatusChanged('tunnel')
        })
      },
      rotateMagicLink: async () => {
        const status = get().status
        if (!status || status.tunnel_state !== 'connected' || !status.public_url) return
        try {
          const issued = await authClient.magicLinkIssue(undefined, undefined)
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
      },
      startMagicLinkRotation: () => {
        if (get().rotationTimer) return
        const timer = setInterval(() => {
          // Skip the tick when the tab is hidden — the QR isn't on screen.
          if (typeof document !== 'undefined' && document.visibilityState === 'hidden') return
          void get().rotateMagicLink()
        }, ROTATION_INTERVAL_MS)
        set(s => {
          s.rotationTimer = timer
        })
      },
      stopMagicLinkRotation: () => {
        const timer = get().rotationTimer
        if (timer) {
          clearInterval(timer)
          set(s => {
            s.rotationTimer = null
          })
        }
      },
    }
  },
  init: ({ actions, onCleanup }) => {
    // Eager-load so the settings page renders with real data on first mount.
    void actions.loadStatus()
    onCleanup(() => actions.stopMagicLinkRotation())
  },
})

export const useRemoteAccessStore = RemoteAccess.store
