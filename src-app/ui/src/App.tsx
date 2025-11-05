import { useMemo, useEffect } from 'react'
import {
  BrowserRouter,
  Routes,
  Route,
  Navigate,
  Outlet,
} from 'react-router-dom'
import { useRouterStore } from './core'
import { AuthGuard } from './modules/auth'
import { ThemeProvider } from './components/ThemeProvider'
import { loadModules } from './modules/loader'
import { setupAccessibilityFixes } from './utils/accessibilityFixes'

// Load all modules before rendering
loadModules()

function App() {
  const { routes } = useRouterStore()

  // Setup global accessibility fixes
  useEffect(() => {
    const cleanup = setupAccessibilityFixes()
    return cleanup
  }, [])

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
                            element={route.element}
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
                        element={route.element}
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
                        element={route.element}
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
                    element={route.element}
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
      </BrowserRouter>
    </ThemeProvider>
  )
}

export default App
