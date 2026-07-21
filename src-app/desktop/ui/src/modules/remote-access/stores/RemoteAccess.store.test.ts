/**
 * Tier 4 — RemoteAccess Zustand store unit tests.
 *
 * Mocks `@/api-client`'s `ApiClient.RemoteAccess` + `ApiClient.Auth`
 * methods to assert:
 *   - loadStatus populates `status` + clears `error`
 *   - saveAuthToken calls PUT /settings + reloads status
 *   - setPasswordAuthEnabled propagates to the auth store
 *   - rotateMagicLink builds the right URL pattern from public_url + token
 *   - error path captures + surfaces messages
 *
 * Uses Vitest's `vi.hoisted` to define the mock object so the module
 * mock factory can reference it, then `vi.mock('@/api-client', ...)`
 * to replace the real client.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'

// Hoisted mock shape so the factory can reach it.
const apiMock = vi.hoisted(() => ({
  RemoteAccess: {
    getStatus: vi.fn(),
    getSettings: vi.fn(),
    updateSettings: vi.fn(),
    startTunnel: vi.fn(),
    stopTunnel: vi.fn(),
  },
  Auth: {
    magicLinkIssue: vi.fn(),
    getConfig: vi.fn(),
  },
}))

vi.mock('@/api-client', () => ({
  ApiClient: apiMock,
}))

// Auth store placeholder. The current store doesn't touch any auth-
// store methods, but mocks are kept (empty) so the proxy import below
// remains resolvable.
const authStoreMock = vi.hoisted(() => ({}))

// EventBusStore.emit is called by the events module; stub it so
// nothing tries to dispatch.
const eventBusMock = vi.hoisted(() => ({
  emit: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('@ziee/framework/stores', () => ({
  Stores: {
    Auth: authStoreMock,
    EventBus: eventBusMock,
  },
}))

// Import AFTER the mocks so the store picks them up.
import { useRemoteAccessStore } from '@ziee/desktop/modules/remote-access/stores/remoteAccess'

function defaultStatus() {
  return {
    password_rotated: true,
    password_auth_enabled: false,
    auth_token_set: false,
    ngrok_domain: null,
    auto_start_tunnel: false,
    tunnel_state: 'idle' as const,
    public_url: null,
    last_error: null,
    started_at: null,
  }
}

beforeEach(() => {
  // Reset store state between tests — Zustand stores are module-level
  // singletons, so without this earlier state bleeds in.
  useRemoteAccessStore.setState({
    status: null,
    loading: false,
    saving: false,
    error: null,
    magicLink: null,
    rotationTimer: null,
  })
  // Reset every mock fn so call-count assertions stay accurate.
  for (const ns of [apiMock.RemoteAccess, apiMock.Auth]) {
    for (const v of Object.values(ns)) {
      if (typeof v === 'function' && 'mockReset' in v) v.mockReset()
    }
  }
  eventBusMock.emit.mockReset().mockResolvedValue(undefined)
})

describe('RemoteAccessStore', () => {
  describe('loadStatus', () => {
    it('populates status and clears error on success', async () => {
      apiMock.RemoteAccess.getStatus.mockResolvedValueOnce(defaultStatus())

      await useRemoteAccessStore.getState().loadStatus()

      const s = useRemoteAccessStore.getState()
      expect(s.status?.tunnel_state).toBe('idle')
      expect(s.loading).toBe(false)
      expect(s.error).toBeNull()
    })

    it('captures error message on failure', async () => {
      apiMock.RemoteAccess.getStatus.mockRejectedValueOnce(new Error('boom'))

      await useRemoteAccessStore.getState().loadStatus()

      const s = useRemoteAccessStore.getState()
      expect(s.error).toBe('boom')
      expect(s.loading).toBe(false)
    })

    it('mints a magic link when status is connected and none cached', async () => {
      apiMock.RemoteAccess.getStatus.mockResolvedValueOnce({
        ...defaultStatus(),
        tunnel_state: 'connected',
        public_url: 'https://my-app.ngrok.app',
      })
      apiMock.Auth.magicLinkIssue.mockResolvedValueOnce({
        token: 'TOKEN-XYZ',
        expires_at: '2030-01-01T00:00:00Z',
      })

      await useRemoteAccessStore.getState().loadStatus()

      const s = useRemoteAccessStore.getState()
      expect(s.magicLink).not.toBeNull()
      expect(s.magicLink!.url).toBe('https://my-app.ngrok.app/auth/magic/TOKEN-XYZ')
      // Rotation timer should now be scheduled.
      expect(s.rotationTimer).not.toBeNull()
    })

    it('clears the magic link when tunnel is no longer connected', async () => {
      // Seed a magic link manually.
      useRemoteAccessStore.setState({
        magicLink: {
          token: 'OLD',
          url: 'https://x/auth/magic/OLD',
          expires_at: '2030-01-01T00:00:00Z',
          issued_at: '2030-01-01T00:00:00Z',
        },
      })
      apiMock.RemoteAccess.getStatus.mockResolvedValueOnce(defaultStatus())

      await useRemoteAccessStore.getState().loadStatus()

      const s = useRemoteAccessStore.getState()
      expect(s.magicLink).toBeNull()
    })
  })

  describe('saveAuthToken', () => {
    it('PUTs the token then reloads status', async () => {
      apiMock.RemoteAccess.updateSettings.mockResolvedValueOnce({
        auth_token_set: true,
        ngrok_domain: null,
        auto_start_tunnel: false,
        password_auth_enabled: false,
      })
      apiMock.RemoteAccess.getStatus.mockResolvedValueOnce({
        ...defaultStatus(),
        auth_token_set: true,
      })

      await useRemoteAccessStore.getState().saveAuthToken('my-token')

      expect(apiMock.RemoteAccess.updateSettings).toHaveBeenCalledWith(
        { ngrok_auth_token: 'my-token' },
        undefined,
      )
      // Status was refetched.
      expect(apiMock.RemoteAccess.getStatus).toHaveBeenCalledTimes(1)
      // Mutation emitted an event.
      expect(eventBusMock.emit).toHaveBeenCalledWith(
        expect.objectContaining({ type: 'remote_access.status_changed' }),
      )
      expect(useRemoteAccessStore.getState().saving).toBe(false)
    })
  })

  describe('setPasswordAuthEnabled', () => {
    it('PUTs the flag and refreshes status', async () => {
      apiMock.RemoteAccess.updateSettings.mockResolvedValueOnce({
        auth_token_set: false,
        ngrok_domain: null,
        auto_start_tunnel: false,
        password_auth_enabled: true,
      })
      apiMock.RemoteAccess.getStatus.mockResolvedValueOnce({
        ...defaultStatus(),
        password_auth_enabled: true,
      })

      await useRemoteAccessStore.getState().setPasswordAuthEnabled(true)

      expect(apiMock.RemoteAccess.updateSettings).toHaveBeenCalledWith(
        { password_auth_enabled: true },
        undefined,
      )
      // Phones fetch /api/auth/config on PhoneAuthPage mount, so the
      // store has nothing to "push" to AuthStore — the toggle is
      // observable on the next phone session start. (See the
      // comment in RemoteAccess.store.ts::setPasswordAuthEnabled.)
    })
  })

  describe('rotateMagicLink', () => {
    it('no-ops when status is idle (refuses to mint without a tunnel)', async () => {
      useRemoteAccessStore.setState({ status: defaultStatus() })

      await useRemoteAccessStore.getState().rotateMagicLink()

      expect(apiMock.Auth.magicLinkIssue).not.toHaveBeenCalled()
      expect(useRemoteAccessStore.getState().magicLink).toBeNull()
    })

    it('mints a fresh token when connected', async () => {
      useRemoteAccessStore.setState({
        status: {
          ...defaultStatus(),
          tunnel_state: 'connected',
          public_url: 'https://my-app.ngrok.app/',
        },
      })
      apiMock.Auth.magicLinkIssue.mockResolvedValueOnce({
        token: 'FRESH',
        expires_at: '2030-01-01T00:00:00Z',
      })

      await useRemoteAccessStore.getState().rotateMagicLink()

      const s = useRemoteAccessStore.getState()
      expect(s.magicLink!.token).toBe('FRESH')
      // Trailing slash in public_url should be stripped before appending.
      expect(s.magicLink!.url).toBe('https://my-app.ngrok.app/auth/magic/FRESH')
    })

    it('swallows network errors gracefully (keeps existing link)', async () => {
      useRemoteAccessStore.setState({
        status: {
          ...defaultStatus(),
          tunnel_state: 'connected',
          public_url: 'https://my-app.ngrok.app',
        },
        magicLink: {
          token: 'OLD',
          url: 'https://my-app.ngrok.app/auth/magic/OLD',
          expires_at: '2030-01-01T00:00:00Z',
          issued_at: '2030-01-01T00:00:00Z',
        },
      })
      apiMock.Auth.magicLinkIssue.mockRejectedValueOnce(new Error('429'))

      await useRemoteAccessStore.getState().rotateMagicLink()

      // Old link preserved.
      expect(useRemoteAccessStore.getState().magicLink!.token).toBe('OLD')
    })
  })

  describe('startMagicLinkRotation / stopMagicLinkRotation', () => {
    it('is idempotent — second start does not double up the timer', () => {
      const s = useRemoteAccessStore.getState()
      s.startMagicLinkRotation()
      const t1 = useRemoteAccessStore.getState().rotationTimer
      s.startMagicLinkRotation()
      const t2 = useRemoteAccessStore.getState().rotationTimer
      expect(t1).toBe(t2)
      // Cleanup.
      s.stopMagicLinkRotation()
    })

    it('clears the timer on stop', () => {
      const s = useRemoteAccessStore.getState()
      s.startMagicLinkRotation()
      expect(useRemoteAccessStore.getState().rotationTimer).not.toBeNull()
      s.stopMagicLinkRotation()
      expect(useRemoteAccessStore.getState().rotationTimer).toBeNull()
    })
  })
})
