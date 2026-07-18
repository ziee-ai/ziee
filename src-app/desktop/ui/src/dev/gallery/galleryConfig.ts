/**
 * ziee-desktop's `GalleryConfig` — the dependency-injection object that binds the
 * generic `@ziee/gallery` framework to the DESKTOP app. Everything the framework
 * used to reach through `@/` (the api-client route table, the module loaders /
 * router store / auth seed, the app ThemeProvider / error boundary / loading +
 * lazy renderers, the accent tokens + theme writers) is supplied here.
 *
 * The desktop canvas is PAGE-FOCUSED (kit stories + overlay/deep/seeded surfaces
 * live in the web workspace), so `stories` / `deepState` are omitted. Surface
 * discovery is the desktop's cross-workspace merge (`module-seed.ts`): the shared
 * web module cassettes + the desktop-only modules' `gallery.tsx`.
 *
 * Mirrors the web workspace's `galleryConfig.ts`; consumed by the standalone
 * `main.tsx` (via `mountGallery`). The desktop gallery has no in-app `/dev/gallery`
 * route, so there is no `GalleryPage.tsx` shim (unlike web).
 */
import type { GalleryConfig } from '@ziee/gallery'
import { Stores } from '@ziee/framework/stores'
import { ApiEndpoints } from '@/api-client/types'
import { ThemeProvider } from '@/components/ThemeProvider'
import { AppErrorBoundary } from '@/components/AppErrorBoundary'
import {
  ACCENT_ORDER,
  ACCENT_PRESETS,
  type AccentPreset,
} from '@/components/ThemeProvider/accentPresets'
import { Loading } from '@/core/components/Loading'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { useRoutesStore } from '@/modules/router/stores'
import { useAuthStore } from '@/modules/auth/Auth.store'
import { loadModules } from '@/modules/loader'
import { loadDesktopModules } from '@ziee/desktop/modules/desktop-loader'
import { crawlCassette } from './fixtures/crawl.generated'
import { adminUser, adminPermissions } from './fixtures'
import { discoverGalleries } from './module-seed'

// ── auth/role seed (was `seed.ts`) ───────────────────────────────────────────
// A conservative read-only permission set a "limited" user plausibly holds.
const LIMITED_PERMISSIONS = [
  'profile::read',
  'chat::read',
  'conversations::read',
  'assistants::read',
]

function seedAuth(auth: 'admin' | 'limited' | 'none'): void {
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
    return
  }
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
    user: limited
      ? { ...adminUser, is_admin: false, username: 'member', display_name: 'Member' }
      : adminUser,
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

// Register core + desktop-specific modules (mirrors the real desktop bootstrap:
// App.tsx loadModules() + main.tsx loadDesktopModules()).
function loadAllModules(): void {
  loadModules()
  loadDesktopModules()
}

/** Build ziee-desktop's full gallery config (idempotent — safe to call twice). */
export function buildGalleryConfig(): GalleryConfig {
  return {
    discoverGalleries,
    apiEndpoints: ApiEndpoints as Record<string, string>,
    crawlCassette,

    loadModules: loadAllModules,
    seedAuth,
    useRoutesStore,
    ThemeProvider,
    ErrorBoundary: AppErrorBoundary,
    Loading,
    LazyComponentRenderer,

    accents: [...ACCENT_ORDER],
    accentLabels: Object.fromEntries(
      ACCENT_ORDER.map(a => [a, ACCENT_PRESETS[a].label]),
    ),
    defaultAccent: 'blue',
    setThemePref: theme => Stores.ConfigClient.setThemePreference(theme),
    setAccentPref: accent =>
      Stores.ConfigClient.setAccentPreset(accent as AccentPreset),

    // Login form renders null when authenticated → seed logged-out for it.
    surfaceAuthSeed: { auth: 'none' },
  }
}
