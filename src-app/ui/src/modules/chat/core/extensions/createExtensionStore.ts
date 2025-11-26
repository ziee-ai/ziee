import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import { subscribeWithSelector } from 'zustand/middleware'
import { createStoreProxy } from '@/core/stores'

/**
 * Create an independent Zustand store for an extension
 * Each extension gets its own store with full reactivity and lifecycle management
 *
 * @param initialState - Default state for the extension
 * @param actions - Extension-specific actions (can access state via get/set)
 * @returns Store proxy for the extension
 *
 * @example
 * ```typescript
 * interface MyExtensionState {
 *   selectedId: string | null
 *   items: Item[]
 * }
 *
 * interface MyExtensionActions {
 *   selectItem: (id: string) => void
 *   loadItems: () => Promise<void>
 * }
 *
 * export const createMyExtensionStore = () =>
 *   createExtensionStore<MyExtensionState, MyExtensionActions>(
 *     { selectedId: null, items: [] },
 *     (set, get) => ({
 *       selectItem: (id: string) => {
 *         set(state => {
 *           state.selectedId = id
 *         })
 *       },
 *       loadItems: async () => {
 *         const items = await fetchItems()
 *         set(state => {
 *           state.items = items
 *         })
 *       }
 *     })
 *   )
 * ```
 */
export function createExtensionStore<
  TState extends Record<string, any>,
  TActions extends Record<string, any> = Record<string, any>,
>(
  initialState: TState,
  actions: (
    set: (fn: (state: TState & TActions) => void) => void,
    get: () => TState & TActions,
  ) => TActions,
) {
  // Create Zustand store with immer and subscribeWithSelector
  const useStore = create<TState & TActions>()(
    subscribeWithSelector(
      immer((set, get) => ({
        // Extension state
        ...initialState,

        // Extension actions
        ...actions(set as any, get as any),
      })),
    ),
  )

  // Wrap with store proxy for reactivity and lifecycle
  return createStoreProxy(useStore)
}

/**
 * Create an empty extension store for stateless extensions
 * Use this for extensions that don't need any state
 *
 * @example
 * ```typescript
 * const myExtension: ChatExtension = createExtension({
 *   name: 'my-extension',
 *   createStore: createEmptyExtensionStore,
 *   // ... other fields
 * })
 * ```
 */
export function createEmptyExtensionStore() {
  return createExtensionStore<Record<string, never>, Record<string, never>>(
    {},
    () => ({}),
  )
}
