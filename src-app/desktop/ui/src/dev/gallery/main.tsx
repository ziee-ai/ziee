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
import { seedGallery } from './seed'
import '@/index.css'

// Seed the gallery: install the mock-API cassette, authenticate an admin, and
// load every module so `Stores.X` resolves for any page and populates through
// the real load() path. This registers ConfigClient (used by ThemeProvider /
// useGalleryTheme) among all other module stores — no manual registration.
seedGallery()

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AppErrorBoundary label="gallery" fallback={() => null}>
      <ThemeProvider>
        <GalleryPage />
      </ThemeProvider>
    </AppErrorBoundary>
  </React.StrictMode>,
)
