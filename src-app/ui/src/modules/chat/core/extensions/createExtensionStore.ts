import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import { subscribeWithSelector } from 'zustand/middleware'
import { createStoreProxy } from '@/core/stores'

/**
 * Create an independent Zustand store for an extension
 * Each extension gets its own store with full reactivity and lifecycle management
 *
 * @param creator - Store creator function that returns state and actions
 * @returns Store proxy for the extension
 *
 * @example
 * ```typescript
 * interface MyExtensionStore {
 *   // State
 *   selectedId: string | null
 *   items: Item[]
 *
 *   // Actions
 *   selectItem: (id: string) => void
 *   loadItems: () => Promise<void>
 * }
 *
 * export const createMyExtensionStore = () =>
 *   createExtensionStore<MyExtensionStore>((set, get) => ({
 *     // State
 *     selectedId: null,
 *     items: [],
 *
 *     // Actions
 *     selectItem: (id: string) => {
 *       set(state => {
 *         state.selectedId = id
 *       })
 *     },
 *     loadItems: async () => {
 *       const items = await fetchItems()
 *       set(state => {
 *         state.items = items
 *       })
 *     }
 *   }))
 * ```
 */
export function createExtensionStore<TStore extends Record<string, any>>(
  creator: (
    set: (fn: (state: TStore) => void) => void,
    get: () => TStore,
  ) => TStore,
) {
  // Create Zustand store with immer and subscribeWithSelector
  const useStore = create<TStore>()(
    subscribeWithSelector(
      immer((set, get) => creator(set as any, get as any)),
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
  return createExtensionStore<Record<string, never>>(() => ({}))
}
