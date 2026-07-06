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
import { OVERLAY_ENTRIES } from './overlays'
import { DEEP_STATE_SLUGS } from './deepStates'
import '@/index.css'

// Runtime manifest for the runtime-health pass. Page slugs are enumerated from
// the rendered DOM (they only exist after the router store populates), but the
// overlay open-states are interaction-only surfaces never present on the browse
// canvas — expose their slugs statically so the health script can drive each via
// `?surface=<slug>&state=open` without hard-coding the list.
;(window as unknown as { __GALLERY_OVERLAYS__?: string[] }).__GALLERY_OVERLAYS__ =
  OVERLAY_ENTRIES.map(o => o.slug)
// Deep active-conversation states (streaming / right-panel / elicitation / …) are
// likewise interaction-only surfaces never on the browse canvas — expose their
// slugs so the health + coverage passes drive each via `?surface=<slug>`.
;(window as unknown as { __GALLERY_DEEP_STATES__?: string[] }).__GALLERY_DEEP_STATES__ =
  DEEP_STATE_SLUGS

// URL-driven multi-state rendering. The DEFAULT (no params) browses every page
// in its loaded state. A single-combo URL renders ONE surface in ONE state for
// per-state screenshots + bug-finding:
//   ?surface=<slug>&state=<loaded|empty|error|delayed>&auth=<admin|limited|none>
const q = new URLSearchParams(window.location.search)
const surface = q.get('surface') ?? undefined
const state = (q.get('state') as MockMode | null) ?? 'loaded'
const auth = (q.get('auth') as AuthSeed | null) ?? (surface ? undefined : 'admin')

// Set the data-state mode BEFORE any store loads (loads are lazy on first read).
setMockMode(state)
// A single-surface render defaults its auth from the surface's needs (login
// flows want `none`); the browse view is always admin.
seedGallery(auth ?? 'admin')

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AppErrorBoundary label="gallery" fallback={() => null}>
      <ThemeProvider>
        <GalleryPage surface={surface} state={state} />
      </ThemeProvider>
    </AppErrorBoundary>
  </React.StrictMode>,
)
