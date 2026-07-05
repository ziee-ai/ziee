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

let seeded = false

export function seedGallery(): void {
  if (seeded) return
  seeded = true

  // 1. Mock backend.
  installMockApi(GALLERY_CASSETTE)

  // 2. Authenticated admin — is_admin short-circuits every permission gate.
  //    A placeholder token satisfies `getAuthToken()` + Auth's own init path
  //    (which would replay `/api/auth/me` through the mock if it runs).
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
    user: adminUser,
    permissions: adminPermissions,
    token: 'gallery-token',
    expiresAt: Date.now() + 24 * 60 * 60 * 1000,
    expiresIn: 24 * 60 * 60,
    hasPassword: true,
    isAuthenticated: true,
    isLoading: false,
    isInitializing: false,
    error: null,
  })

  // 3. Register core modules (the desktop `@/modules/loader` fork) AND the
  //    desktop-specific modules (updater, tunnel-auth, host-mount, remote-access,
  //    window, …) — mirroring the real desktop bootstrap (App.tsx + main.tsx).
  loadModules()
  loadDesktopModules()
}
