import type { StoreApi, UseBoundStore } from 'zustand'
import { useShallow } from 'zustand/react/shallow'
import { useRouterStore } from './router'

// ============================================================================
// Store Proxy - Creates typed store accessors with IntelliSense
// ============================================================================

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
  return new Proxy({} as Readonly<ExtractZustandState<T>>, {
    get: (_, prop) => {
      if (prop === '__state') {
        return useStore.getState()
      }
      if (prop === '__setState') {
        return useStore.setState.bind(useStore)
      }

      const isInit = propInitCheck.get(prop) || false
      if (!isInit) {
        let state = useStore.getState()
        if (state.__init__ && typeof state.__init__[prop] === 'function') {
          state.__init__[prop]()
        }
        propInitCheck.set(prop, true)
      }

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
    const routerState = useRouterStore.getState()
    return routerState.stores[prop as string]
  },
})

// Type helper for accessing store state
export type StoresType = RegisteredStores
