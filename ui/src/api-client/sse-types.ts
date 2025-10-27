// SSE event types for reuse across the API layer
export type SSEEventKey<T> = T extends Record<string, any>
  ? T extends any[] | Function
    ? '__init'
    : keyof T extends string
      ? keyof T | '__init'
      : '__init'
  : '__init'

export type SSEEventData<T, K extends SSEEventKey<T>> = K extends '__init'
  ? { abortController: AbortController }
  : T extends Record<string, any>
    ? K extends keyof T
      ? T[K]
      : never
    : never

// Generic SSE event handler with proper type narrowing
// Export the handlers type for explicit use
export type SSEHandlers<T extends Record<string, any>> = {
  [K in keyof T]: (data: T[K]) => void
} & {
  __init?: (data: { abortController: AbortController }) => void
  default?: (event: string, data: any) => void
}

// Unified SSE callback type that supports both function and object formats
export type SSECallback<T> =
  | (<E extends SSEEventKey<T>>(event: E, data: SSEEventData<T, E>) => void)
  | ((event: string, data: any) => void)
  | (T extends Record<string, any> ? Partial<SSEHandlers<T>> : never)

export function createSSEHandler<T extends Record<string, any>>(
  handlers: SSEHandlers<T>,
) {
  return (event: SSEEventKey<T>, data: any) => {
    if (event === '__init' && handlers.__init) {
      handlers.__init(data as { abortController: AbortController })
    } else if (event in handlers && event !== '__init') {
      const handler = handlers[event as keyof T]
      if (handler) {
        handler(data)
      }
    } else if (handlers.default) {
      handlers.default(event as string, data)
    }
  }
}
