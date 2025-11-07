import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { EventHandler, Unsubscribe, AppEvents, BaseEvent } from './types'

interface EventBusState {
  // Map of event type to set of handlers (use any for storage, type-safe at call site)
  handlers: Map<string, Set<EventHandler<any>>>

  // Subscribe to specific event type (type-safe)
  on: <K extends keyof AppEvents>(
    eventType: K,
    handler: EventHandler<AppEvents[K]>,
  ) => Unsubscribe

  // Emit event to all subscribed handlers (type-safe)
  emit: <K extends keyof AppEvents>(
    event: Omit<AppEvents[K], 'timestamp'>,
  ) => Promise<void>

  // Remove specific handler
  off: (eventType: string, handler: EventHandler<any>) => void

  // Clear all handlers (for testing)
  clear: () => void

  // Get handler count for debugging
  getHandlerCount: (eventType?: string) => number
}

export const useEventBusStore = create<EventBusState>()(
  subscribeWithSelector(
    immer((set, get): EventBusState => ({
      handlers: new Map(),

      on: (eventType, handler) => {
        set(state => {
          if (!state.handlers.has(eventType)) {
            state.handlers.set(eventType, new Set())
          }
          state.handlers.get(eventType)!.add(handler)
        })

        // Return unsubscribe function
        return () => {
          get().off(eventType, handler)
        }
      },

      emit: async <K extends keyof AppEvents>(
        event: Omit<AppEvents[K], 'timestamp'>,
      ) => {
        // Add timestamp automatically
        const fullEvent = {
          ...event,
          timestamp: Date.now(),
        } as AppEvents[K] & BaseEvent

        // Extract event type - safe because all events must have 'type' property
        const eventType = (event as any).type as string

        const { handlers } = get()
        const eventHandlers = handlers.get(eventType)

        if (!eventHandlers || eventHandlers.size === 0) {
          return
        }

        console.debug(`[EventBus] Emitting: ${eventType}`, fullEvent)

        // Execute all handlers (non-blocking)
        const promises = Array.from(eventHandlers).map(handler => {
          try {
            return Promise.resolve(handler(fullEvent))
          } catch (error) {
            console.error(
              `[EventBus] Handler error for ${eventType}:`,
              error,
            )
            return Promise.resolve()
          }
        })

        await Promise.all(promises)
      },

      off: (eventType, handler) => {
        set(state => {
          const eventHandlers = state.handlers.get(eventType)
          if (eventHandlers) {
            eventHandlers.delete(handler)
            if (eventHandlers.size === 0) {
              state.handlers.delete(eventType)
            }
          }
        })
      },

      clear: () => {
        set(state => {
          state.handlers.clear()
        })
      },

      getHandlerCount: (eventType?: string) => {
        const { handlers } = get()
        if (eventType) {
          return handlers.get(eventType)?.size || 0
        }
        return Array.from(handlers.values()).reduce(
          (sum, set) => sum + set.size,
          0,
        )
      },
    })),
  ),
)
