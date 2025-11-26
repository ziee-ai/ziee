import type {
  ComponentType,
  ReactNode,
  ReactElement,
  LazyExoticComponent,
} from 'react'

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
  /** Route path (e.g., "/chat", "/settings") */
  path: string

  /** Route element - React component (lazy or eager) */
  element:
    | ReactElement
    | LazyExoticComponent<ComponentType<any>>
    | (() => Promise<{ default: ComponentType<any> }>)

  /** Whether route requires authentication (default: false) */
  requiresAuth?: boolean

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

export {}
