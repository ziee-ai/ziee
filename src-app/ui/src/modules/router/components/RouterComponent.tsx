import type { ReactNode } from 'react'
import {
  BrowserRouter,
  Routes,
  Route,
  Navigate,
  Outlet,
} from 'react-router-dom'
import { Result, Spin } from 'antd'
import { Stores } from '@/core/stores'
import { AuthGuard } from '@/modules/auth'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { usePermission } from '@/core/permissions'
import type { PermissionExpr } from '@/core/permissions'
import type { LayoutDefinition, RouteConfig } from '@/modules/router/types'

/**
 * Private router-level gate. Renders an inline 403 panel in place
 * of the route element when the current user fails the route's
 * `permission` expression — URL and layout stay intact.
 */
function RoutePermissionGate({
  permission,
  children,
}: {
  permission: PermissionExpr
  children: ReactNode
}) {
  const allowed = usePermission(permission)
  if (!allowed) {
    return (
      <Result
        status="403"
        title="Not authorized"
        subTitle="You don't have permission to view this page."
      />
    )
  }
  return <>{children}</>
}

const ROUTE_SPINNER = (
  <div className="h-full w-full flex items-center justify-center">
    <Spin size="large" />
  </div>
)

/**
 * Materializes a route's element: feeds `route.element` through
 * `LazyComponentRenderer` and wraps with `RoutePermissionGate` when
 * `route.permission` is set.
 */
function renderRouteElement(route: RouteConfig<any>) {
  const inner = (
    <LazyComponentRenderer
      component={route.element}
      fallback={ROUTE_SPINNER}
    />
  )
  if (!route.permission) return inner
  return (
    <RoutePermissionGate permission={route.permission}>
      {inner}
    </RoutePermissionGate>
  )
}

/**
 * RouterComponent - Handles all routing logic for the application.
 *
 * Responsibilities:
 * - Groups routes by auth requirement (protected vs public)
 * - Groups routes by layout
 * - Merges layout options from all routes using a layout
 * - Wraps protected routes with AuthGuard
 * - Renders routes with their layouts
 */
export function RouterComponent() {
  const { routes } = Stores.Routes

  // Group routes by auth requirement
  const protectedRoutes = routes.filter(r => r.requiresAuth)
  const publicRoutes = routes.filter(r => !r.requiresAuth)

  /**
   * Renders a list of routes, grouping them by layout.
   * For routes with the same layout, creates a single layout instance with nested routes.
   * For routes without a layout, renders them directly.
   */
  const renderRoutesForLayoutGroup = (routeList: RouteConfig<any>[]) => {
    // Group by layout
    const routesByLayout = new Map<
      LayoutDefinition<any> | null,
      RouteConfig<any>[]
    >()

    routeList.forEach(route => {
      const layoutKey = route.layout || null
      if (!routesByLayout.has(layoutKey)) {
        routesByLayout.set(layoutKey, [])
      }
      routesByLayout.get(layoutKey)!.push(route)
    })

    return Array.from(routesByLayout.entries()).map(
      ([layoutDef, layoutRoutes]) => {
        if (!layoutDef) {
          // No layout - render routes directly
          return layoutRoutes.map(route => (
            <Route
              key={route.path}
              path={route.path}
              element={renderRouteElement(route)}
              index={route.index}
            />
          ))
        }

        // Render layout with nested routes
        const LayoutComponent = layoutDef.component

        return (
          <Route
            key={layoutDef.component.name || 'layout'}
            element={
              <LayoutComponent>
                <Outlet />
              </LayoutComponent>
            }
          >
            {layoutRoutes.map(route => (
              <Route
                key={route.path}
                path={route.path}
                element={renderRouteElement(route)}
                index={route.index}
              />
            ))}
          </Route>
        )
      },
    )
  }

  return (
    <BrowserRouter>
      <Routes>
        {/* Protected routes with AuthGuard */}
        {protectedRoutes.length > 0 && (
          <Route
            element={
              <AuthGuard>
                <Outlet />
              </AuthGuard>
            }
          >
            {renderRoutesForLayoutGroup(protectedRoutes)}
          </Route>
        )}

        {/* Public routes */}
        {renderRoutesForLayoutGroup(publicRoutes)}

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
  )
}
