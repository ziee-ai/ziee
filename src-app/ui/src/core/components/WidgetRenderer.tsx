import { Suspense, lazy, isValidElement, useMemo } from 'react'
import { Spin } from 'antd'

interface WidgetRendererProps {
  widget: {
    component: React.ComponentType<any> | (() => Promise<{ default: React.ComponentType<any> }>) | React.ReactElement
  }
  props?: Record<string, any>
}

/**
 * Renders a widget component, handling lazy loading automatically.
 *
 * Supports three component types:
 * 1. Lazy-loaded functions (from lazyWithPreload)
 * 2. Regular React components
 * 3. Already-rendered React elements
 *
 * Automatically wraps lazy components in Suspense with a loading fallback.
 */
export function WidgetRenderer({ widget, props = {} }: WidgetRendererProps) {
  const renderedComponent = useMemo(() => {
    // If it's already a valid React element, return as-is
    if (isValidElement(widget.component)) {
      return widget.component
    }

    // If it's a lazy function (from lazyWithPreload), wrap with lazy
    if (typeof widget.component === 'function' && widget.component.length === 0) {
      const LazyComponent = lazy(widget.component as () => Promise<{ default: React.ComponentType<any> }>)
      return <LazyComponent {...props} />
    }

    // Otherwise it's a regular component
    const Component = widget.component as React.ComponentType<any>
    return <Component {...props} />
  }, [widget.component, props])

  // Wrap in Suspense for lazy components
  if (typeof widget.component === 'function' && widget.component.length === 0) {
    return (
      <Suspense
        fallback={
          <div className="p-3 flex justify-center">
            <Spin size="small" />
          </div>
        }
      >
        {renderedComponent}
      </Suspense>
    )
  }

  return <>{renderedComponent}</>
}
