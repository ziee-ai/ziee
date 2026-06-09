import React from 'react'
import ReactDOM from 'react-dom/client'
import { App } from '@ziee/ui-core'
import { Stores } from '@/core/stores'
import { loadDesktopModules } from '@ziee/desktop/modules/desktop-loader'
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

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
