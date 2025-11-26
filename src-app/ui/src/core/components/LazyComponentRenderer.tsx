import {
  Suspense,
  lazy,
  isValidElement,
  useMemo,
  type ReactNode,
  type ComponentType,
  type ReactElement,
} from 'react'
import { Spin } from 'antd'

type LazyComponent = () => Promise<{ default: ComponentType<any> }>
type ComponentLike = ComponentType<any> | LazyComponent | ReactElement

interface LazyComponentRendererProps {
  /**
   * Component to render - can be:
   * 1. Lazy-loaded function (from lazyWithPreload)
   * 2. Regular React component
   * 3. Already-rendered React element
   */
  component: ComponentLike

  /**
   * Props to pass to the component (ignored if component is ReactElement)
   */
  props?: Record<string, any>

  /**
   * Custom fallback to show while lazy loading
   * @default <Spin size="small" /> with padding
   */
  fallback?: ReactNode
}

/**
 * Universal component renderer with automatic lazy loading support.
 *
 * Handles three component types:
 * 1. Lazy-loaded functions (from lazyWithPreload) - wrapped in Suspense
 * 2. Regular React components - rendered directly
 * 3. Already-rendered React elements - passed through as-is
 *
 * Automatically wraps lazy components in Suspense with a configurable fallback.
 *
 * @example
 * ```tsx
 * // Lazy component with default spinner
 * <LazyComponentRenderer component={LazyWidget} props={{ id: 1 }} />
 *
 * // Custom fallback
 * <LazyComponentRenderer component={LazyRoute} fallback={<Spin size="large" />} />
 *
 * // No fallback
 * <LazyComponentRenderer component={LazyApp} fallback={null} />
 * ```
 */
export function LazyComponentRenderer({
  component,
  props = {},
  fallback = (
    <div className="p-3 flex justify-center">
      <Spin size="small" />
    </div>
  ),
}: LazyComponentRendererProps) {
  // Check if it's a lazy function by checking if it's a function with 0 params
  // AND doesn't have React component characteristics (like a name or prototype properties)
  const isLikelyLazy =
    typeof component === 'function' &&
    component.length === 0 &&
    !component.prototype?.isReactComponent && // Not a class component
    (!component.name || component.name === '') // Lazy functions from lazyWithPreload are usually anonymous

  const renderedComponent = useMemo(() => {
    // If it's already a valid React element, return as-is
    if (isValidElement(component)) {
      return component
    }

    // If it looks like a lazy function, wrap with lazy
    if (isLikelyLazy) {
      const LazyComponent = lazy(component as LazyComponent)
      return <LazyComponent {...props} />
    }

    // Otherwise it's a regular component - render it directly
    const Component = component as ComponentType<any>
    return <Component {...props} />
  }, [component, props, isLikelyLazy])

  // Wrap in Suspense for lazy components
  if (isLikelyLazy) {
    return <Suspense fallback={fallback}>{renderedComponent}</Suspense>
  }

  return <>{renderedComponent}</>
}

// Legacy export for backwards compatibility
/**
 * @deprecated Use LazyComponentRenderer instead
 */
export function WidgetRenderer({
  widget,
  props,
}: {
  widget: { component: ComponentLike }
  props?: Record<string, any>
}) {
  return <LazyComponentRenderer component={widget.component} props={props} />
}
