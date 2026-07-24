import { Suspense, useEffect, useState, type ComponentType, type ReactNode } from 'react'
import {
  BrowserRouter,
  Routes,
  Route,
  Navigate,
  Outlet,
  Link,
  useLocation,
} from 'react-router-dom'
import { Button, Result } from '@ziee/kit'
import { ModuleSystem } from '@ziee/framework/stores'
import { useRoutesStore } from '@/modules/router/stores/routes-store'
import { ensureModuleForPath, isPathModulePending, isPathModuleForbidden, revalidateForPath } from '@/modules/loader'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { Loading } from '@/core/components/Loading'
import { usePermission } from '@/core/permissions'
import type { PermissionExpr } from '@/core/permissions'
import type { LayoutDefinition, RouteConfig } from '@/modules/router/types'

/**
 * Private router-level gate. Renders an inline 403 panel in place
 * of the route element when the current user fails the route's
 * `permission` expression — URL and layout stay intact.
 */
/** The inline router-level 403 panel (URL + layout stay intact). */
function ForbiddenResult() {
  return (
    <Result
      data-testid="router-route-forbidden-result"
      status="403"
      title="Not authorized"
      subtitle="You don't have permission to view this page."
      extra={
        <Link to="/">
          <Button data-testid="router-403-back-home-btn" variant="default">Back to home</Button>
        </Link>
      }
    />
  )
}

function RoutePermissionGate({
  permission,
  children,
}: {
  permission: PermissionExpr
  children: ReactNode
}) {
  const allowed = usePermission(permission)
  if (!allowed) return <ForbiddenResult />
  return <>{children}</>
}

const ROUTE_SPINNER = <Loading />


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
/**
 * Route-driven safety net (smart loading): on navigation to a path no
 * currently-registered module owns, load the manifest module that DOES own it
 * (a deep-link that arrives before the reactive load wave, or a page whose
 * predicate hasn't fired). No-op when a loaded module already owns the path or
 * nothing in the manifest matches (a genuine 404). The freshly-registered
 * module's routes appear reactively, so the route resolves on the next render.
 */
function RouteModuleLoader() {
  const location = useLocation()
  useEffect(() => {
    // ensureModuleForPath: load the module that OWNS this route (deep-link net).
    // revalidateForPath: re-evaluate shouldLoad against the new path so
    // location-scoped modules that own no route load on arrival. NOTE: this
    // effect is held while a lazy route suspends the transition, so a page whose
    // OWN content comes from location-scoped modules also calls revalidateForPath
    // from its own mount effect (see HubPage).
    void ensureModuleForPath(location.pathname)
    revalidateForPath(location.pathname)
  }, [location.pathname])
  return null
}

/**
 * Fallback element for the `*` route (smart loading). A hard-reload / bookmark
 * straight onto a lazy-module route arrives BEFORE that module's reactive load
 * wave, so no route matches yet and this `*` branch is hit. Redirecting here
 * (the old behavior) discarded the deep-link. Instead: if an eligible manifest
 * module OWNS this path and is still loading, render a spinner and WAIT — the
 * module's routes appear on the next render (routes-store update re-renders the
 * router) and the real route takes over. Only redirect once the path is settled
 * with no match (a genuine 404) — or after a bounded timeout guards against a
 * failed chunk load spinning forever.
 */
function RouteFallback({ children }: { children: ReactNode }) {
  const location = useLocation()
  // Re-render when routes change (a module registering its routes flows through
  // useRoutesStore in the parent) — this component reads the parent's render.
  const pending = isPathModulePending(location.pathname)
  // A real route the user LACKS permission for: its owning module is deliberately
  // NOT loaded (the ensureModuleForPath security guard — the gated code is never
  // delivered), so no route ever matches. Render the in-place 403 here (URL
  // preserved) rather than redirecting home, so an unauthorized deep-link keeps
  // its address and explains itself.
  const forbidden = isPathModuleForbidden(location.pathname)
  const [timedOut, setTimedOut] = useState(false)

  useEffect(() => {
    setTimedOut(false)
    if (!pending) return
    // Safety net: a failed module load leaves the path "pending" forever
    // (its name is removed from `loaded` on error). Bound the wait so a broken
    // chunk redirects home instead of spinning indefinitely.
    const t = setTimeout(() => setTimedOut(true), 10000)
    return () => clearTimeout(t)
  }, [location.pathname, pending])

  if (pending && !timedOut) return <Loading fullscreen />
  if (forbidden) return <ForbiddenResult />
  return <>{children}</>
}

export function RouterComponent() {
  const routes = useRoutesStore(s => s.routes) as RouteConfig<any>[]

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
      ([layoutDef, layoutRoutes], layoutIdx) => {
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

        // Render layout with nested routes. The layout `component` may be a
        // React.lazy ref (layout shells are lazily loaded so referencing a
        // LayoutDefinition doesn't pull the shell into boot) — wrap in Suspense.
        // `layoutIdx` keys it because a lazy component has no stable `.name`.
        const LayoutComponent = layoutDef.component

        return (
          <Route
            key={`layout-${layoutIdx}`}
            element={
              <Suspense fallback={<Loading fullscreen />}>
                <LayoutComponent>
                  <Outlet />
                </LayoutComponent>
              </Suspense>
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
  const routerEffects = (ModuleSystem.slots.get('routerEffects') ||
    []) as Array<{ id: string; component: ComponentType }>

  // Route guards contributed by features (auth fills this). The router owns
  // the slot type and composes the guards; it does NOT import any guard.
  const guards = (ModuleSystem.slots.get('routeGuards') ||
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
      <RouteModuleLoader />
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

        {/* Fallback route (same guard so unknown deep links hit login). Wrapped
            in RouteFallback so a deep-link onto a not-yet-loaded lazy-module
            route WAITS for that module instead of being redirected away. */}
        <Route
          path="*"
          element={guardProtected(
            <RouteFallback>
              <Navigate to="/" replace />
            </RouteFallback>,
          )}
        />
      </Routes>
    </BrowserRouter>
  )
}
