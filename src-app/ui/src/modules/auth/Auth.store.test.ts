/**
 * TEST-14..18 — session teardown on logout / user-switch.
 *
 * The bug these pin: logging out left the PREVIOUS user's state in the store,
 * so after logging in as a different user in the same tab the new user still
 * saw the admin's conversations (verified live: the sidebar read "bob" while
 * listing the admin's chats). The wipe also omitted `permissions`, so every
 * permission gate (<Can>, usePermission, hasPermissionNow) kept evaluating the
 * previous user's grants.
 *
 *   - TEST-14  logoutUser wipes the session INCLUDING permissions + reloads once
 *   - TEST-15  desktop (refreshFallback set) never reloads — no login page to
 *              land on; auto_login re-mints instead
 *   - TEST-16  a terminal refresh 401 tears down + reloads
 *   - TEST-17  a terminal refresh 401 WITH a desktop fallback re-mints instead
 *              of reloading (pins the existing guard's ordering)
 *   - TEST-18  setAuthFromAutoLogin PRESERVES permissions on a same-identity
 *              re-mint (desktop/tunnel), and TEST-18b pins why that is safe:
 *              the only identity-changing caller hands over user: null and
 *              stays unauthenticated until /me resolves
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

const apiMock = vi.hoisted(() => ({
  Auth: {
    logout: vi.fn(() => Promise.resolve({})),
    refresh: vi.fn(),
    me: vi.fn(),
  },
}))

const bus = vi.hoisted(() => ({
  on: () => () => {},
  removeGroupListeners: () => {},
}))

vi.mock('@/api-client', () => ({
  ApiClient: apiMock,
  setUnauthorizedHandler: vi.fn(),
  isTauri: () => false,
}))
vi.mock('@/core/stores', () => ({
  Stores: { EventBus: { emit: vi.fn(() => Promise.resolve()) } },
  createStoreProxy: () => ({}),
}))
vi.mock('@/core/events', () => ({
  useEventBusStore: { getState: () => bus },
}))

import { useAuthStore } from './Auth.store'

const state = () => useAuthStore.getState()

/** A session as it looks for a logged-in admin. */
function seedAdminSession() {
  useAuthStore.setState({
    user: { id: 'admin-1', username: 'admin' } as never,
    token: 'admin-token',
    expiresAt: Date.now() + 3_600_000,
    expiresIn: 3600,
    permissions: ['users::read', 'users::edit', '*'],
    hasPassword: true,
    isAuthenticated: true,
    isLoading: false,
    isInitializing: false,
    error: null,
  })
}

let reload: ReturnType<typeof vi.fn>

beforeEach(() => {
  vi.clearAllMocks()
  reload = vi.fn()
  // jsdom's window.location.reload is not configurable-assignable; replace the
  // whole accessor so the store's call is observable.
  Object.defineProperty(window, 'location', {
    configurable: true,
    value: { ...window.location, reload },
  })
  state().setRefreshFallback(null)
  seedAdminSession()
})

describe('logoutUser', () => {
  it('TEST-14: wipes the session INCLUDING permissions, and reloads once', async () => {
    await state().logoutUser()

    expect(state().token).toBeNull()
    expect(state().user).toBeNull()
    expect(state().isAuthenticated).toBe(false)
    // The omission that let a logged-out tab keep rendering admin UI.
    expect(state().permissions).toEqual([])
    expect(state().hasPassword).toBe(false)
    // The reload is what discards every OTHER store's per-user data.
    expect(reload).toHaveBeenCalledTimes(1)
  })

  it('TEST-15: does NOT reload on desktop (refreshFallback registered)', async () => {
    state().setRefreshFallback(async () => {})

    await state().logoutUser()

    expect(reload).not.toHaveBeenCalled()
    // State is still cleared — only the reload is suppressed.
    expect(state().token).toBeNull()
    expect(state().permissions).toEqual([])
  })
})

describe('terminal refresh 401', () => {
  it('TEST-16: tears down and reloads (web)', async () => {
    apiMock.Auth.refresh.mockRejectedValueOnce(
      Object.assign(new Error('unauthorized'), { status: 401 }),
    )

    const ok = await state().refreshSession()

    expect(ok).toBe(false)
    expect(state().isAuthenticated).toBe(false)
    expect(state().token).toBeNull()
    expect(state().permissions).toEqual([])
    expect(reload).toHaveBeenCalledTimes(1)
  })

  it('TEST-17: desktop re-mints via the fallback instead of reloading', async () => {
    apiMock.Auth.refresh.mockRejectedValueOnce(
      Object.assign(new Error('unauthorized'), { status: 401 }),
    )
    state().setRefreshFallback(async () => {
      useAuthStore.setState({ token: 're-minted', isAuthenticated: true })
    })

    const ok = await state().refreshSession()

    expect(ok).toBe(true)
    expect(state().token).toBe('re-minted')
    // Reloading a Tauri window would strand it: AuthGuard.desktop renders no
    // login page.
    expect(reload).not.toHaveBeenCalled()
  })
})

describe('setAuthFromAutoLogin', () => {
  it('TEST-18: preserves permissions for a same-identity re-mint (desktop/tunnel)', () => {
    // The callers that reach this path — desktop `applyTokens` and the tunnel
    // `applySession` — re-mint the SAME identity and never call initAuth()
    // afterwards (AuthGuard.desktop skips it by design). Clearing permissions
    // here would therefore strand them with `[]` for the rest of the session,
    // silently disabling every permission-gated surface for any non-admin
    // identity (admins only survive it via the is_admin short-circuit).
    // The one caller that DOES change identity (the OAuth callback) passes
    // user: null and takes the early-return above, so it never lands here.
    state().setAuthFromAutoLogin({
      access_token: 'new-token',
      refresh_token: 'new-refresh',
      expires_in: 3600,
      user: { id: 'admin-1', username: 'admin' },
    } as never)

    expect(state().isAuthenticated).toBe(true)
    expect(state().token).toBe('new-token')
    expect(state().permissions).toEqual(['users::read', 'users::edit', '*'])
    expect(state().hasPassword).toBe(true)
  })

  it('TEST-18b: an identity-changing callback stays unauthenticated until /me resolves', () => {
    // Guards the reason TEST-18 is safe: the OAuth path hands over user: null,
    // so no authenticated render can observe the previous identity's grants.
    state().setAuthFromAutoLogin({
      access_token: 'oauth-token',
      refresh_token: '',
      expires_in: 3600,
      user: null,
    } as never)

    expect(state().isAuthenticated).toBe(false)
    expect(state().user).toBeNull()
  })
})
