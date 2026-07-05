/**
 * Gallery bootstrap: install the mock-API cassette + seed an authenticated
 * admin, then load every module so any page's stores resolve and populate.
 *
 * Order matters:
 *  1. install the fetch interceptor BEFORE any store can fire a load;
 *  2. seed the Auth store (admin) so permission gates (`hasPermissionNow`)
 *     short-circuit and permission-gated loads actually run;
 *  3. load modules (registers every store + route; store `init` stays lazy
 *     and fires when a rendered page first reads `Stores.X`).
 *
 * Idempotent — safe to call once from the standalone entry or the in-app route.
 */
import { useAuthStore } from '@/modules/auth/Auth.store'
import { loadModules } from '@/modules/loader'
import { loadDesktopModules } from '@ziee/desktop/modules/desktop-loader'
import { GALLERY_CASSETTE, adminUser, adminPermissions } from './fixtures'
import { installMockApi } from './mockApi'

/**
 * Auth/role seed for permission-state coverage:
 *   - admin   : is_admin → every gate open (the default);
 *   - limited : a non-admin user with a minimal read-only permission set, to
 *               surface permission-gated empty / hidden-action / 403 states;
 *   - none    : logged out, for the auth/login/setup flows.
 */
export type AuthSeed = 'admin' | 'limited' | 'none'

// A conservative read-only permission set a "limited" user plausibly holds.
const LIMITED_PERMISSIONS = [
  'profile::read',
  'chat::read',
  'conversations::read',
  'assistants::read',
]

let seeded = false

export function seedGallery(auth: AuthSeed = 'admin'): void {
  if (seeded) return
  seeded = true

  // 1. Mock backend.
  installMockApi(GALLERY_CASSETTE)

  // 2. Auth/role seed. is_admin short-circuits every permission gate; a limited
  //    user exercises the gated paths; none drives the logged-out flows.
  if (auth === 'none') {
    try {
      // eslint-disable-next-line no-undef
      localStorage.removeItem('auth-storage')
    } catch {}
    useAuthStore.setState({
      user: null,
      permissions: [],
      token: null,
      isAuthenticated: false,
      isLoading: false,
      isInitializing: false,
      error: null,
    })
  } else {
    const limited = auth === 'limited'
    try {
      // eslint-disable-next-line no-undef
      localStorage.setItem(
        'auth-storage',
        JSON.stringify({ state: { token: 'gallery-token' }, version: 0 }),
      )
    } catch {
      // Non-browser / restricted storage — the direct setState below is enough.
    }
    useAuthStore.setState({
      user: limited ? { ...adminUser, is_admin: false, username: 'member', display_name: 'Member' } : adminUser,
      permissions: limited ? LIMITED_PERMISSIONS : adminPermissions,
      token: 'gallery-token',
      expiresAt: Date.now() + 24 * 60 * 60 * 1000,
      expiresIn: 24 * 60 * 60,
      hasPassword: true,
      isAuthenticated: true,
      isLoading: false,
      isInitializing: false,
      error: null,
    })
  }

  // 3. Register core + desktop-specific modules (mirrors the real desktop
  //    bootstrap: App.tsx loadModules() + main.tsx loadDesktopModules()).
  loadModules()
  loadDesktopModules()
}
