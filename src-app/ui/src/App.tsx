import { AppShell } from '@ziee/shell'
import { useAuthStore } from '@/modules/auth/Auth.store'
import { loadModules } from '@/modules/loader'

// Discover + register all app modules (glob-based; app-owned) BEFORE rendering.
loadModules()

/**
 * App — thin ziee consumer of `@ziee/shell`'s `AppShell`.
 *
 * The generic shell body (ThemeProvider, per-module error boundaries, the
 * order-sorted module render, the realtime-sync wiring) lives in `@ziee/shell`.
 * ziee supplies only what's app-specific: module discovery (`loadModules`) and
 * its auth store (wired to the sync SSE lifecycle).
 */
function App() {
  return <AppShell authStore={useAuthStore} />
}

export default App
