import { useMemo, useEffect, Suspense, isValidElement, lazy } from 'react'
import {
  BrowserRouter,
  Routes,
  Route,
  Navigate,
  Outlet,
} from 'react-router-dom'
import { Spin } from 'antd'
import { Stores } from './core/stores'
import { AuthGuard } from './modules/auth'
import { ThemeProvider } from './components/ThemeProvider'
import { GlobalComponentRenderer } from './core/components/GlobalComponentRenderer'
import { loadModules } from './modules/loader'
import { setupAccessibilityFixes } from './utils/accessibilityFixes'
import { usePrefetchModules } from './hooks/usePrefetchModules'
import type { RouteConfig } from './core/router/types'

// Load all modules before rendering
loadModules()

// Helper function to wrap route elements in Suspense for lazy loading
function wrapWithSuspense(element: RouteConfig['element']) {
  // If it's already a valid React element, return as-is
  if (isValidElement(element)) {
    return element
  }

  // Otherwise, it's a function (preload function from lazyWithPreload), wrap with lazy
  const LazyComponent = lazy(element as () => Promise<{ default: React.ComponentType<any> }>)
  return (
    <Suspense
      fallback={
        <div className="h-full w-full flex items-center justify-center">
          <Spin size="large" />
        </div>
      }
    >
      <LazyComponent />
    </Suspense>
  )
}

function App() {
  const { routes } = Stores.Router

  // Setup global accessibility fixes
  useEffect(() => {
    const cleanup = setupAccessibilityFixes()
    return cleanup
  }, [])

  // Prefetch lazy-loaded modules when browser is idle
  usePrefetchModules()

  // Memoized route grouping by requiresAuth and layout
  const { hasProtectedRoutes, protectedByLayout, publicByLayout } =
    useMemo(() => {
      const protected_ = routes.filter(r => r.requiresAuth)
      const public_ = routes.filter(r => !r.requiresAuth)

      const groupByLayout = (routeList: typeof routes) => {
        const grouped = new Map<any, typeof routes>()

        routeList.forEach(route => {
          const layoutKey = route.layout || null
          if (!grouped.has(layoutKey)) {
            grouped.set(layoutKey, [])
          }
          grouped.get(layoutKey)!.push(route)
        })

        return grouped
      }

      return {
        hasProtectedRoutes: protected_.length > 0,
        protectedByLayout: groupByLayout(protected_),
        publicByLayout: groupByLayout(public_),
      }
    }, [routes])

  return (
    <ThemeProvider>
      <BrowserRouter>
        <Routes>
          {/* Single AuthGuard for all protected routes */}
          {hasProtectedRoutes && (
            <Route
              element={
                <AuthGuard>
                  <Outlet />
                </AuthGuard>
              }
            >
              {Array.from(protectedByLayout.entries()).map(
                ([Layout, layoutRoutes]) => {
                  if (Layout) {
                    // Routes with layout: create single layout instance with nested routes
                    return (
                      <Route
                        key={Layout.name || 'layout'}
                        element={
                          <Layout>
                            <Outlet />
                          </Layout>
                        }
                      >
                        {layoutRoutes.map(route => (
                          <Route
                            key={route.path}
                            path={route.path}
                            element={wrapWithSuspense(route.element)}
                            index={route.index}
                          />
                        ))}
                      </Route>
                    )
                  } else {
                    // Routes without layout: direct children
                    return layoutRoutes.map(route => (
                      <Route
                        key={route.path}
                        path={route.path}
                        element={wrapWithSuspense(route.element)}
                        index={route.index}
                      />
                    ))
                  }
                },
              )}
            </Route>
          )}

          {/* Public routes - no AuthGuard */}
          {Array.from(publicByLayout.entries()).map(
            ([Layout, layoutRoutes]) => {
              if (Layout) {
                // Routes with layout: create single layout instance with nested routes
                return (
                  <Route
                    key={Layout.name || 'layout'}
                    element={
                      <Layout>
                        <Outlet />
                      </Layout>
                    }
                  >
                    {layoutRoutes.map(route => (
                      <Route
                        key={route.path}
                        path={route.path}
                        element={wrapWithSuspense(route.element)}
                        index={route.index}
                      />
                    ))}
                  </Route>
                )
              } else {
                // Routes without layout: direct children
                return layoutRoutes.map(route => (
                  <Route
                    key={route.path}
                    path={route.path}
                    element={wrapWithSuspense(route.element)}
                    index={route.index}
                  />
                ))
              }
            },
          )}

          {/* Fallback route */}
          <Route
            path="*"
            element={
              <AuthGuard>
                <Navigate to="/" replace />
              </AuthGuard>
            }
          />
        </Routes>

        {/* Global components from all modules */}
        <GlobalComponentRenderer />
      </BrowserRouter>
    </ThemeProvider>
  )
}

export default App
