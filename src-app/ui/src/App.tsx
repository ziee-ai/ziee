import { useMemo, useEffect } from 'react'
import { ThemeProvider } from './components/ThemeProvider'
import { loadModules } from './modules/loader'
import { setupAccessibilityFixes } from './utils/accessibilityFixes'
import { usePrefetchModules } from './hooks/usePrefetchModules'
import { Stores } from './core/stores'
import { LazyComponentRenderer } from './core/components/LazyComponentRenderer'

// Load all modules before rendering
loadModules()

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

  console.log({components})

  // Prefetch lazy-loaded modules when browser is idle
  usePrefetchModules()

  // Sort components by order (RouterComponent has order: 0, so it renders first)
  const sortedComponents = useMemo(() => {
    return [...components].sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
  }, [components])

  return (
    <ThemeProvider>
      {/* Render components from modules (sorted by order) */}
      {sortedComponents.map(comp => (
        <LazyComponentRenderer  key={comp.id} component={comp.component} fallback={null} />
      ))}
    </ThemeProvider>
  )
}

export default App
