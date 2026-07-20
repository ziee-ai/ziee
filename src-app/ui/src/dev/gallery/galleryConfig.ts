/**
 * ziee's `GalleryConfig` — the dependency-injection object that binds the generic
 * `@ziee/gallery` framework to this app. Everything the framework used to reach
 * through `@/` (the api-client route table, the module loader / router store /
 * auth seed, the app ThemeProvider / error boundary / loading + lazy renderers,
 * the accent tokens + theme writers, the app-specific SSE / binary / text
 * endpoints) is supplied here.
 *
 * Consumed by BOTH the standalone `main.tsx` (via `mountGallery`) and the in-app
 * `/dev/gallery` route (via the `GalleryPage.tsx` shim, which sets the config
 * without installing the mock — the in-app route renders against the live app).
 */
import { lazy } from 'react'
import {
  type GalleryConfig,
  type SpecialRoute,
  base64ToBytes,
  jsonResponse,
  makeBinaryResponse,
  mockErrorResponse,
  sseReplayResponse,
} from '@ziee/gallery'
import { Stores } from '@ziee/framework/stores'
import { ApiEndpoints } from '@/api-client/apiEndpoints'
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
import { SAMPLE_PDF_BASE64 } from '@/modules/file/viewers/pdf/pdf-fixture'
import { ALL_STORIES } from './stories'
import { crawlCassette } from './fixtures/crawl.generated'
import { adminUser, adminPermissions } from './fixtures'
import {
  firstEnabledRemoteProviderId,
  llmProvidersList,
} from './fixtures/llm-providers'
import { discoverGalleries } from './support/registry'

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

// ── app-specific mock endpoints (was inline in `mockApi.ts`) ─────────────────
const SSE_SUBSCRIPTION = /\/api\/chat\/stream\/subscription$/
const SSE_STREAM = /\/api\/chat\/stream$/
const FILE_RAW = /^\/api\/files\/[^/]+\/raw$/
const FILE_TEXT = /^\/api\/files\/[^/]+\/text$/
const SAMPLE_CANVAS_MD =
  '# Assay Methods\n\nSamples were prepared with **care** and *precision* using `buffer A`.\n\n- RNA extraction\n- Reverse transcription\n\n> Keep samples on ice.\n'

const specialRoutes: SpecialRoute[] = [
  // The subscription PUT is a no-op 200.
  { test: p => SSE_SUBSCRIPTION.test(p), respond: () => jsonResponse({}) },
  // Chat-token SSE stream: replay the recorded frame cassette as a real
  // text/event-stream (set via `setSseCassette` by the chat gallery).
  { test: (p, m) => SSE_STREAM.test(p) && m === 'GET', respond: () => sseReplayResponse() },
  // Binary raw-file bytes (PDF viewer). `error` mode still yields a 500 so the
  // viewer's error state is reachable.
  {
    test: (p, m) => FILE_RAW.test(p) && m === 'GET',
    respond: ({ mode }) =>
      mode === 'error'
        ? mockErrorResponse(500)
        : makeBinaryResponse(base64ToBytes(SAMPLE_PDF_BASE64), 'application/pdf'),
  },
  // Extracted text for the artifact canvas editor (FileEditBody).
  {
    test: (p, m) => FILE_TEXT.test(p) && m === 'GET',
    respond: () =>
      new Response(SAMPLE_CANVAS_MD, {
        status: 200,
        headers: { 'content-type': 'text/plain; charset=utf-8' },
      }),
  },
]

/** Build ziee's full gallery config (idempotent — safe to call more than once). */
export function buildGalleryConfig(): GalleryConfig {
  return {
    discoverGalleries,
    apiEndpoints: ApiEndpoints as Record<string, string>,
    crawlCassette,
    specialRoutes,

    loadModules,
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

    stories: ALL_STORIES,
    paramValues: {
      providerId: firstEnabledRemoteProviderId ?? llmProvidersList.providers[0]?.id,
    },
    deepState: {
      component: lazy(() => import('@/modules/chat/pages/ConversationPage')),
      routePath: '/chat/:conversationId',
      buildInitialPath: conversationId => `/chat/${conversationId}`,
    },
    surfaceAuthSeed: { auth: 'none' },
  }
}
