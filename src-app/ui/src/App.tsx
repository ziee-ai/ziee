import { useEffect, useMemo } from 'react'
import { AppErrorBoundary } from '@/components/AppErrorBoundary'
import { ThemeProvider } from '@/components/ThemeProvider'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import type { ComponentRegistration } from '@/core/module-system/types'
import { Stores } from '@/core/stores'
import { initSync } from '@/core/sync'
import { usePrefetchModules } from '@/hooks/usePrefetchModules'
import { useAuthStore } from '@/modules/auth/Auth.store'
import { loadModules } from '@/modules/loader'
import { setupAccessibilityFixes } from '@/utils/accessibilityFixes'

// Load all modules before rendering
loadModules()

/**
 * ConditionalComponent - Wrapper that checks shouldMount hook before rendering
 */
function ConditionalComponent({
  registration,
}: {
  registration: ComponentRegistration
}) {
  // Call shouldMount hook if provided, default to true
  const shouldMount = registration.shouldMount?.() ?? true

  // Memoize the component renderer to prevent recreating it when shouldMount changes
  const renderer = useMemo(
    () => (
      <LazyComponentRenderer
        component={registration.component}
        fallback={null}
      />
    ),
    [registration.component],
  )

  if (!shouldMount) {
    return null
  }

  return renderer
}

/**
 * App - Main application component
 *
 * Phase 2: Meta-Framework Architecture
 * - RouterComponent (from router module) handles all routing logic
 * - App.tsx just renders components from modules in order
 * - No routing knowledge here - it's all in RouterComponent
 */
function App() {
  const { components } = Stores.ModuleSystem

  // Setup global accessibility fixes
  useEffect(() => {
    const cleanup = setupAccessibilityFixes()
    return cleanup
  }, [])

  // Wire the realtime-sync SSE stream to the auth lifecycle (idempotent;
  // starts on login, stops on logout, restarts on user switch).
  useEffect(() => {
    initSync(useAuthStore)
  }, [])

  // Prefetch lazy-loaded modules when browser is idle
  usePrefetchModules()

  // Sort components by order (RouterComponent has order: 0, so it renders first)
  const sortedComponents = useMemo(() => {
    return [...components].sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
  }, [components])

  return (
    <ThemeProvider>
      {/* Render components from modules (sorted by order). Each module is
       * wrapped in its own ErrorBoundary so a single module crash
       * isolates to that module — other modules + the shell keep
       * working. The outer boundary in main.tsx catches anything that
       * escapes this layer (e.g. a ThemeProvider throw). */}
      {sortedComponents.map(comp => (
        <AppErrorBoundary key={comp.id} label={comp.id} fallback={() => null}>
          <ConditionalComponent registration={comp} />
        </AppErrorBoundary>
      ))}
    </ThemeProvider>
  )
}

export default App
