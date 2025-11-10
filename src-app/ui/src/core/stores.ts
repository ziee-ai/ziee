import type { StoreApi, UseBoundStore } from 'zustand'
import { useShallow } from 'zustand/react/shallow'
import { useEffect } from 'react'
import { useModuleSystemStore } from './module-system'

// ============================================================================
// Store Proxy - Creates typed store accessors with IntelliSense
// ============================================================================

// Default delay before destroying a store (5 seconds)
const DEFAULT_DESTROY_DELAY_MS = 5000

// Reference tracking interface
export interface ReferenceTracker {
  counts: Map<string | symbol, number>
  totalCount: number
  destroyTimeoutId: NodeJS.Timeout | null
  destroyed: boolean
  addRef: (prop: string | symbol) => void
  removeRef: (prop: string | symbol) => void
  hasRefs: () => boolean
  scheduleDestroy: () => void
  cancelDestroy: () => void
  executeDestroy: () => void
  reset: () => void
}

type RemoveVoid<T> = T extends void ? never : T

type ExtractZustandState<T> = T extends UseBoundStore<infer Store>
  ? Store extends StoreApi<infer State>
    ? RemoveVoid<State> & {
        __state: RemoveVoid<State>
        __setState: StoreApi<State>['setState']
      }
    : Store extends { getState(): infer State }
      ? State extends void | infer S
        ? S extends void
          ? never
          : S
        : RemoveVoid<State> & {
            __state: RemoveVoid<State>
            __setState: any
          }
      : never
  : never

export const createStoreProxy = <T extends UseBoundStore<StoreApi<any>>>(
  useStore: T,
): Readonly<ExtractZustandState<T>> => {
  const propInitCheck = new Map<string | symbol, boolean>()
  let storeInitialized = false

  // Reference tracking with delayed destruction
  const refTracker: ReferenceTracker = {
    counts: new Map<string | symbol, number>(),
    totalCount: 0,
    destroyTimeoutId: null,
    destroyed: false,

    addRef: (prop: string | symbol) => {
      // If destruction is pending, cancel it (user is accessing again!)
      if (refTracker.destroyTimeoutId !== null) {
        if (import.meta.env.DEV) {
          console.log('🔄 Cancelling destruction - store accessed again')
        }
        refTracker.cancelDestroy()
      }

      // If store was destroyed, reset for re-initialization
      if (refTracker.destroyed) {
        if (import.meta.env.DEV) {
          console.log('🔄 Re-initializing previously destroyed store')
        }
        refTracker.reset()
      }

      const current = refTracker.counts.get(prop) || 0
      refTracker.counts.set(prop, current + 1)
      refTracker.totalCount++
    },

    removeRef: (prop: string | symbol) => {
      const current = refTracker.counts.get(prop) || 0
      if (current > 0) {
        refTracker.counts.set(prop, current - 1)
        refTracker.totalCount--

        // Schedule destruction when no active references
        if (refTracker.totalCount === 0) {
          refTracker.scheduleDestroy()
        }
      }
    },

    hasRefs: () => refTracker.totalCount > 0,

    scheduleDestroy: () => {
      const state = useStore.getState()

      // Only schedule if store has __destroy__ method
      if (!state.__destroy__ || typeof state.__destroy__ !== 'function') {
        return
      }

      // Get custom delay from store or use default
      const delay = (state as any).__destroyDelay__ || DEFAULT_DESTROY_DELAY_MS

      if (import.meta.env.DEV) {
        console.log(
          `⏳ Scheduling store destruction in ${delay}ms (no active references)`,
        )
      }

      refTracker.destroyTimeoutId = setTimeout(() => {
        refTracker.executeDestroy()
      }, delay)
    },

    cancelDestroy: () => {
      if (refTracker.destroyTimeoutId !== null) {
        clearTimeout(refTracker.destroyTimeoutId)
        refTracker.destroyTimeoutId = null
      }
    },

    executeDestroy: () => {
      const state = useStore.getState()

      if (import.meta.env.DEV) {
        console.log('🗑️ Executing store destruction (delay expired)')
      }

      try {
        // Call store's custom destroy hook
        const result = (state as any).__destroy__()
        if (result instanceof Promise) {
          result.catch((err: any) => {
            console.error('Store __destroy__ error:', err)
          })
        }

        // Mark as destroyed and clear initialization state immediately
        refTracker.destroyed = true
        refTracker.destroyTimeoutId = null

        // Clear initialization state so store can be re-initialized if accessed again
        propInitCheck.clear()
        storeInitialized = false

        if (import.meta.env.DEV) {
          console.log('✅ Store destroyed successfully')
        }
      } catch (err) {
        console.error('Store __destroy__ error:', err)
      }
    },

    reset: () => {
      // Reset initialization flags for re-initialization
      propInitCheck.clear()
      storeInitialized = false
      refTracker.destroyed = false
      refTracker.totalCount = 0
      refTracker.counts.clear()

      if (import.meta.env.DEV) {
        console.log('🔄 Store tracker reset for re-initialization')
      }
    },
  }

  return new Proxy({} as Readonly<ExtractZustandState<T>>, {
    get: (_, prop) => {
      // Special properties
      if (prop === '__state') {
        return useStore.getState()
      }
      if (prop === '__setState') {
        return useStore.setState.bind(useStore)
      }
      if (prop === '__refCount') {
        return refTracker.totalCount
      }
      if (prop === '__refTracker') {
        return refTracker
      }
      if (prop === '__destroyed') {
        return refTracker.destroyed
      }

      const state = useStore.getState()

      // Store-level initialization (only if not destroyed)
      if (!storeInitialized && state.__init__?.__store__) {
        if (typeof state.__init__.__store__ === 'function') {
          state.__init__.__store__()
        }
        storeInitialized = true
      }

      // Property-specific initialization
      const isInit = propInitCheck.get(prop) || false
      if (!isInit) {
        if (state.__init__ && typeof state.__init__[prop] === 'function') {
          state.__init__[prop]()
        }
        propInitCheck.set(prop, true)
      }

      // If the property is a function (action), return it directly
      const value = (state as any)[prop]
      if (typeof value === 'function') {
        return value
      }

      // For state values, track reference with useEffect
      // eslint-disable-next-line react-hooks/rules-of-hooks
      useEffect(() => {
        // Component mounted and accessing this property
        refTracker.addRef(prop)

        // Cleanup when component unmounts
        return () => {
          refTracker.removeRef(prop)
        }
      }, []) // Empty deps - only run on mount/unmount

      // Return reactive value via hook
      return useStore(
        useShallow((state: ExtractZustandState<T>) => (state as any)[prop]),
      )
    },
  })
}

// ============================================================================
// Registered Stores - Dynamic store registry with IntelliSense
// ============================================================================

// Helper type to wrap store state with proxy methods
export type StoreProxy<T> = Readonly<
  T & {
    __state: T
    __setState: (partial: Partial<T> | ((state: T) => Partial<T>)) => void
    __refCount: number
    __refTracker: ReferenceTracker
    __destroyed: boolean
  }
>

// This interface will be augmented by modules via declaration merging
export interface RegisteredStores {
  // Modules will add their store types here via:
  // declare module '@/core/stores' {
  //   interface RegisteredStores {
  //     Auth: StoreProxy<{ user: User, isAuthenticated: boolean, ... }>
  //   }
  // }
}

// Dynamic store proxy that gets populated by modules at runtime
// But typed via RegisteredStores interface for IntelliSense
export const Stores = new Proxy({} as RegisteredStores, {
  get: (_, prop) => {
    const moduleSystemState = useModuleSystemStore.getState()
    return moduleSystemState.stores[prop as string]
  },
})

// Type helper for accessing store state
export type StoresType = RegisteredStores
