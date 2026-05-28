import React from 'react'
import ReactDOM from 'react-dom/client'
import { App } from '@ziee/ui-core'
import { loadDesktopModules } from '@ziee/desktop/modules/desktop-loader'
import { installDecorumTitlebarFix } from '@/core/decorum-titlebar-fix'
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

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
