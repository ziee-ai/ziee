/**
 * Standalone (backend-free) entry for the component gallery.
 *
 * Served by the Vite dev server at `/dev-gallery.html` (Vite root is `src/`).
 * Unlike the in-app `/dev/gallery` route, this does NOT boot the module system,
 * auth, or any backend call — it registers ONLY the `ConfigClient` store (which
 * the app `ThemeProvider` reads) and mounts the real `ThemeProvider` + gallery.
 * That makes it a deterministic, fully static canvas for the Playwright layout +
 * screenshot layers.
 */
import React from 'react'
import ReactDOM from 'react-dom/client'
import { ThemeProvider } from '@/components/ThemeProvider'
import { AppErrorBoundary } from '@/components/AppErrorBoundary'
import { GalleryPage } from './GalleryPage'
import { seedGallery, type AuthSeed } from './seed'
import { setMockMode, type MockMode } from './mockApi'
import '@/index.css'

// Runtime manifest for the runtime-health pass (mirrors the web gallery). The
// desktop gallery is PAGE-focused — kit component stories + interaction-only
// overlay open-states live in the web workspace — so there are no extra overlay
// surfaces to drive here. Page slugs are still enumerated from the rendered DOM
// by the health script; this empty manifest just tells it there are no
// `?surface=<slug>&state=open` overlay cells on this canvas.
;(window as unknown as { __GALLERY_OVERLAYS__?: string[] }).__GALLERY_OVERLAYS__ =
  []

// Surfaces whose CONTENT only renders under a specific auth seed, so a capture
// that drives `?surface=<slug>` without an explicit `&auth=` still gets the
// reviewable state. `/auth` (the login form) returns null when authenticated —
// it needs a logged-out seed or it renders blank. An explicit `?auth=` wins.
const SURFACE_AUTH_SEED: Record<string, AuthSeed> = {
  auth: 'none',
}

// URL-driven multi-state rendering. The DEFAULT (no params) browses every page
// in its loaded state. A single-combo URL renders ONE surface in ONE state for
// per-state screenshots + bug-finding:
//   ?surface=<slug>&state=<loaded|empty|error|delayed>&auth=<admin|limited|none>
const q = new URLSearchParams(window.location.search)
const surface = q.get('surface') ?? undefined
const state = (q.get('state') as MockMode | null) ?? 'loaded'
// Auth precedence: explicit `?auth=` → per-surface default → admin.
const auth =
  (q.get('auth') as AuthSeed | null) ??
  (surface ? SURFACE_AUTH_SEED[surface] : undefined) ??
  'admin'

// Set the data-state mode BEFORE any store loads (loads are lazy on first read).
setMockMode(state)
// A single-surface render defaults its auth from the surface's needs (login
// flows want `none`); the browse view is always admin.
seedGallery(auth)

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AppErrorBoundary label="gallery" fallback={() => null}>
      <ThemeProvider>
        <GalleryPage surface={surface} state={state} />
      </ThemeProvider>
    </AppErrorBoundary>
  </React.StrictMode>,
)
