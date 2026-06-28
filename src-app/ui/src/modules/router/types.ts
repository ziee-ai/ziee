import type {
  ComponentType,
  ReactNode,
  ReactElement,
  LazyExoticComponent,
} from 'react'
import type { PermissionExpr } from '@/core/permissions'

/**
 * LayoutDefinition defines a layout component.
 * Layouts receive their configuration via the slot system, not through route options.
 */
export interface LayoutDefinition<TOptions = any> {
  /** The layout component that wraps route content */
  component: ComponentType<{ children: ReactNode }>

  /** Legacy field for compatibility (unused) */
  mergeOptions: (routes: RouteConfig<any>[]) => TOptions
}

/**
 * RouteConfig defines a route with optional layout.
 * Layouts receive their configuration via the slot system, not through route options.
 *
 * @template TLayout - The layout definition type (undefined if no layout)
 */
export interface RouteConfig<
  TLayout extends LayoutDefinition<any> | undefined = undefined,
> {
  /**
   * Route path (e.g., "/chat", "/settings").
   * Supports React Router dynamic segments (":param") and optional
   * segments (":param?"), e.g. "/chat/:conversationId",
   * "/projects/:projectId", "/settings/llm-providers/:providerId?".
   */
  path: string

  /** Route element - React component (lazy or eager) */
  element:
    | ReactElement
    | LazyExoticComponent<ComponentType<any>>
    | (() => Promise<{ default: ComponentType<any> }>)

  /** Whether route requires authentication (default: false) */
  requiresAuth?: boolean

  /**
   * Optional permission expression. When set, the router wraps the
   * route element with a gate that renders an inline 403 panel if
   * the current user fails the expression (URL preserved, layout
   * preserved). See `.claude/PERMISSION_GATING.md`.
   */
  permission?: PermissionExpr

  /** Whether this is an index route */
  index?: boolean

  /** Layout to use for this route (optional) */
  layout?: TLayout
}

/**
 * Extend CreateModuleOptions to add routes field.
 * This is how the Router module adds routing capability to all modules.
 */
declare module '@/core/module' {
  interface CreateModuleOptions {
    routes?: RouteConfig<any>[] // Router module adds this!
  }
}

/**
 * `routerEffects` is the router's own extension point: a list of headless,
 * effect-only components that RouterComponent mounts INSIDE <BrowserRouter>
 * (so they can use useNavigate/useLocation), each rendering null and doing
 * its work in a useEffect. The slot's TYPE is owned here by the consumer
 * (the router), not by any plugin that fills it — so the router type-checks
 * standalone and removing a contributing module just empties the list.
 * Plugins (e.g. onboarding) populate it at runtime via their module.tsx.
 */
declare module '@/core/module-system/types' {
  interface Slots {
    routerEffects: Array<{ id: string; component: ComponentType }>
    /**
     * Route guards wrapping every `requiresAuth` route. The router owns this
     * type and composes the registered guards; features (auth) fill it via
     * their module.tsx `slots`. The FIRST-registered guard is the OUTERMOST
     * wrapper (its redirect short-circuits before inner guards mount). An
     * empty slot is sealed fail-closed by RouterComponent — protected routes
     * are never rendered ungated.
     */
    routeGuards: Array<{
      id: string
      component: ComponentType<{ children: ReactNode }>
    }>
  }
}

export {}
