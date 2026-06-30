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
import { createModule } from '@/core'
import { useModuleSystemStore } from '@/core/module-system'
import { useConfigClientStore } from '@/modules/config-client/ConfigClient.store'
import { GalleryPage } from './GalleryPage'
import '@/index.css'

// Register just the ConfigClient store so `Stores.ConfigClient` resolves and the
// real ThemeProvider/useGalleryTheme path works without the full app bootstrap.
// `createModule` normalizes `stores` into the `registerStores()` the module
// system reads (a raw object would be silently ignored).
useModuleSystemStore.getState().registerModule(
  createModule({
    metadata: {
      name: 'gallery-standalone',
      version: '1.0.0',
      description: 'Standalone gallery store host',
    },
    stores: [{ name: 'ConfigClient', store: useConfigClientStore }],
  }),
)

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AppErrorBoundary label="gallery" fallback={() => null}>
      <ThemeProvider>
        <GalleryPage />
      </ThemeProvider>
    </AppErrorBoundary>
  </React.StrictMode>,
)
