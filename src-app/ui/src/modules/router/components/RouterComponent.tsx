import type { ComponentType, ReactNode } from 'react'
import {
  BrowserRouter,
  Routes,
  Route,
  Navigate,
  Outlet,
} from 'react-router-dom'
import { Result, Spin } from 'antd'
import { Stores } from '@/core/stores'
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
 * - Wraps protected routes with the registered `routeGuards`
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

  // Effect-only components contributed by other modules that need to
  // mount INSIDE the router (so they can use `useNavigate` /
  // `useLocation`). Each slot entry is a `ComponentType` rendered
  // alongside <Routes>. The component should return null and do its
  // work in a useEffect. Used by e.g. the onboarding module to handle
  // its own redirect logic without auth or router needing to know
  // about it.
  const routerEffects = (Stores.ModuleSystem.slots.get('routerEffects') ||
    []) as Array<{ id: string; component: ComponentType }>

  // Route guards contributed by features (auth fills this). The router owns
  // the slot type and composes the guards; it does NOT import any guard.
  const guards = (Stores.ModuleSystem.slots.get('routeGuards') ||
    []) as Array<{ id: string; component: ComponentType<{ children: ReactNode }> }>

  if (guards.length === 0 && protectedRoutes.length > 0) {
    // Fail-closed: a guard is a security control, so — unlike routerEffects —
    // an empty slot must NOT render protected routes ungated. In practice
    // this only happens if the auth module failed to register.
    console.error(
      '[router] No routeGuards registered; protected routes are sealed. ' +
        'Did the auth module fail to load?',
    )
  }

  // Wrap `inner` in the registered guards (first-registered = outermost).
  // When no guard is registered, seal protected content to the login wall.
  const guardProtected = (inner: ReactNode): ReactNode =>
    guards.length > 0
      ? guards.reduceRight<ReactNode>(
          (acc, g) => <g.component key={g.id}>{acc}</g.component>,
          inner,
        )
      : <Navigate to="/auth" replace />

  return (
    <BrowserRouter>
      {routerEffects.map(({ id, component: Effect }) => (
        <Effect key={id} />
      ))}
      <Routes>
        {/* Protected routes, wrapped in the registered routeGuards */}
        {protectedRoutes.length > 0 && (
          <Route element={guardProtected(<Outlet />)}>
            {renderRoutesForLayoutGroup(protectedRoutes)}
          </Route>
        )}

        {/* Public routes */}
        {renderRoutesForLayoutGroup(publicRoutes)}

        {/* Fallback route (same guard so unknown deep links hit login) */}
        <Route path="*" element={guardProtected(<Navigate to="/" replace />)} />
      </Routes>
    </BrowserRouter>
  )
}
