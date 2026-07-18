import React from 'react'
import ReactDOM from 'react-dom/client'
import { App } from '@ziee/ui-core'
import { Stores } from '@ziee/framework/stores'
import { loadDesktopModules } from '@ziee/desktop/modules/desktop-loader'
import { registerDesktopOverrides } from '@ziee/desktop/modules/desktop-base/overrides'
import { AppErrorBoundary } from '@/components/AppErrorBoundary'
import { Button } from '@ziee/kit'
// Use the explicit `@ziee/desktop/*` alias — `@/*` resolves against
// core UI per tsconfig `paths`, so a desktop-only file isn't reachable
// via `@/core/...` at typecheck time even though Vite's
// localOverridePlugin handles it at runtime.
import { installDecorumTitlebarFix } from '@ziee/desktop/core/decorum-titlebar-fix'
import '@/index.css'

// Posthoc CSS patch for tauri-plugin-decorum's z:100 titlebar overlay
// (Windows only). Without it, the SidebarToggleButton at top-left is
// unclickable because decorum's drag region paints above it. No-op on
// macOS / web. See decorum-titlebar-fix.ts for the threat model.
installDecorumTitlebarFix()

/**
 * Desktop Application Entry Point
 *
 * Core UI modules are registered by App.tsx's top-level
 * `loadModules()` side effect — which the localOverridePlugin
 * routes to `desktop/ui/src/modules/loader.ts` (the desktop fork
 * with `CORE_MODULE_BLOCKLIST`). Don't call `loadCoreModules` here
 * too: it would re-run the unfiltered core loader and re-register
 * blocklisted modules (registerModule de-dupes existing names but
 * the blocklisted ones haven't been registered yet — they'd sneak
 * in via the second call).
 *
 * Desktop-specific modules (window, tray, file-dialog, etc.) are
 * loaded explicitly below, after core registration completes
 * (which happens at App import time via the side effect above).
 */

// Load desktop-specific modules (window, tray, file-dialog, etc.)
console.log('Loading desktop modules...')
loadDesktopModules()

// Flip the portable multi-user flag synchronously BEFORE any React
// render so core in-page widgets that key off it (MCP user-policy
// card, MCP groups-assignment card, future single-admin-irrelevant
// widgets) never render in their multi-user form on desktop. Doing
// this in `desktop-base/module.tsx::initialize()` (async) would
// leave a brief render-flash window before the flip; setting it
// here happens after both core + desktop modules have registered
// their stores (`Stores.AppMode` from `modules/app/module.tsx`) and
// before `createRoot().render(<App/>)`.
Stores.AppMode.setMultiUserMode(false)

// Register every element-level desktop UI override (a `<Seam>` declared in a
// core web component) BEFORE the first render, so a core component that reads
// its seam resolves the desktop variant on its very first paint. Same
// pre-render window as `setMultiUserMode(false)` above.
registerDesktopOverrides()

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AppErrorBoundary
      label="root"
      fallback={(error, reset) => (
        <div
          role="alert"
          style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            minHeight: '100dvh',
            padding: 24,
            gap: 16,
            fontFamily:
              '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
          }}
        >
          <h1 style={{ margin: 0, fontSize: 24 }}>Something went wrong</h1>
          <p
            data-allow-custom-color
            style={{ margin: 0, color: '#666', maxWidth: 480, textAlign: 'center' }}
          >
            The application encountered an unexpected error. You can try
            again, or reload the page if the problem persists.
          </p>
          <pre
            data-allow-custom-color
            style={{
              margin: 0,
              padding: 12,
              background: '#f5f5f5',
              border: '1px solid #ddd',
              borderRadius: 4,
              fontSize: 12,
              maxWidth: 600,
              overflow: 'auto',
            }}
          >
            {error.message}
          </pre>
          <div style={{ display: 'flex', gap: 12 }}>
            <Button
              data-testid="desktop-error-boundary-retry"
              onClick={reset}
            >
              Try again
            </Button>
            <Button
              data-testid="desktop-error-boundary-reload"
              variant="outline"
              onClick={() => window.location.reload()}
            >
              Reload page
            </Button>
          </div>
        </div>
      )}
    >
      <App />
    </AppErrorBoundary>
  </React.StrictMode>,
)
