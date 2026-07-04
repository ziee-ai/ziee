import { create, useStore } from 'zustand'
import { createStore } from 'zustand/vanilla'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer as immerMiddleware } from 'zustand/middleware/immer'
import { useShallow } from 'zustand/react/shallow'
import { useEffect, useRef } from 'react'
import type { Mutate, StoreApi, UseBoundStore } from 'zustand'
import { useEventBusStore } from '@/core/events'
import type { AppEvents, EventHandler, Unsubscribe } from '@/core/events/types'

// ============================================================================
// store-kit — thin authoring layer over the existing Zustand + Stores.X proxy.
//
// Goal: kill the per-store ceremony (create()(subscribeWithSelector(immer(…))),
// the __init__/__destroy__ magic keys, the GROUP string + manual
// removeGroupListeners, the {name,store} runtime entry) WITHOUT changing how
// consumers read stores. `const { a, b } = Stores.X` and `Stores.X.action()`
// are unchanged; `Stores.X.$` is the clean handler-side snapshot (see the proxy).
// ============================================================================

/** Anything with the subscribeWithSelector 3-arg `subscribe` + `getState`
 *  (a raw Zustand store, or another store's `.store`). Used by `watch`. */
export interface Subscribable<S> {
  getState: () => S
  subscribe: {
    (listener: (state: S, prev: S) => void): Unsubscribe
    <U>(
      selector: (state: S) => U,
      listener: (u: U, prev: U) => void,
      options?: { equalityFn?: (a: U, b: U) => boolean; fireImmediately?: boolean },
    ): Unsubscribe
  }
}

/** The `set` handed to actions/init. Accepts an immer-draft mutator (when
 *  `immer: true`) OR a partial / merge-updater (plain Zustand). Typed loosely
 *  on purpose so both styles compile; the runtime middleware enforces the real
 *  contract per store. */
export type StoreSet<S> = (
  partial: Partial<S> | ((draft: S) => Partial<S> | void),
) => void

/** Base toolkit passed to `init` (plus `actions`, added in StoreConfig). */
export interface StoreInitCtx<State> {
  set: StoreSet<State>
  get: () => State
  /** Subscribe to an app event (auto-grouped by the store name, auto-cleaned). */
  on: <K extends keyof AppEvents>(
    event: K,
    handler: EventHandler<AppEvents[K]>,
  ) => void
  /** React to another store's slice (replaces raw `useX.subscribe`, auto-cleaned).
   *  Fires immediately by default. */
  watch: <S, U>(
    store: Subscribable<S>,
    selector: (s: S) => U,
    cb: (value: U, prev: U) => void,
    opts?: { fireImmediately?: boolean; equalityFn?: (a: U, b: U) => boolean },
  ) => void
  /** Register an arbitrary teardown to run on store destroy — the escape hatch
   *  for imperative resources (an SSE AbortController, a timer) that aren't an
   *  `on`/`watch` subscription. Runs alongside the auto-cleaned listeners. */
  onCleanup: (fn: () => void) => void
}

export interface StoreConfig<State extends object, Actions extends object> {
  /** Draft-mutation setters (`set(d => { d.x = 1 })`). Default false → plain
   *  Zustand shallow-merge (`set(s => ({ x: 1 }))`), so merge-style stores like
   *  Chat migrate with NO change to their setters. */
  immer?: boolean
  state: State
  // `get`/`set` are typed over STATE only (not State & Actions) — that keeps
  // `Actions` out of any parameter position, so it's inferred cleanly from the
  // return (and consumers get full action typing). An action that calls another
  // action does so via a local closure; `init` gets the resolved `actions`.
  actions?: (set: StoreSet<State>, get: () => State) => Actions
  /** Runs once on first access (global) / on mount (local). Listener + cross-store
   *  wiring goes here via `on` / `watch`; all of it auto-unsubscribes on destroy.
   *  Gets the resolved `actions` so it can call them (typed). */
  init?: (ctx: StoreInitCtx<State> & { actions: Actions }) => void
}

/** Internal lifecycle keys the Stores proxy already understands. */
interface Lifecycle {
  __init__: { __store__: () => void }
  __destroy__: () => void
}

export type FullStoreState<State, Actions> = State & Actions & Lifecycle

/** A registered global store: shaped exactly as `StoreRegistration`
 *  ({ name, store }) so `createModule({ stores: [handle] })` accepts it. */
/** The store bound-hook type, carrying the `subscribeWithSelector` mutator so
 *  the 3-arg `store.subscribe(selector, cb, opts)` overload is preserved for
 *  consumers (chat extensions, `watch`) — every store-kit store is wrapped in
 *  subscribeWithSelector at runtime. */
export type BoundStore<FullState> = UseBoundStore<
  Mutate<StoreApi<FullState>, [['zustand/subscribeWithSelector', never]]>
>

/** A registered global store: shaped exactly as `StoreRegistration`
 *  ({ name, store }) so `createModule({ stores: [handle] })` accepts it. */
export interface StoreHandle<FullState> {
  name: string
  store: BoundStore<FullState>
}

/** Build the state object + wire the auto-cleanup lifecycle. Shared by the
 *  global (`defineStore`) and local (`defineLocalStore`) factories. */
function makeBuilder<State extends object, Actions extends object>(
  groupName: string,
  config: StoreConfig<State, Actions>,
) {
  return (set: any, get: any): FullStoreState<State, Actions> => {
    const actions = config.actions ? config.actions(set, get) : ({} as Actions)
    const cleanups: Unsubscribe[] = []
    const ctx: StoreInitCtx<State> & { actions: Actions } = {
      set,
      get,
      actions,
      on: (event, handler) => {
        const busOn = useEventBusStore.getState().on as (
          e: string,
          h: EventHandler<any>,
          g?: string,
        ) => Unsubscribe
        cleanups.push(busOn(event as string, handler as EventHandler<any>, groupName))
      },
      watch: (store, selector, cb, opts) => {
        cleanups.push(
          (store.subscribe as any)(selector, cb, { fireImmediately: true, ...opts }),
        )
      },
      onCleanup: fn => {
        cleanups.push(fn)
      },
    }
    return {
      ...(config.state as State),
      ...actions,
      __init__: { __store__: () => config.init?.(ctx) },
      __destroy__: () => {
        cleanups.splice(0).forEach(off => {
          try {
            off()
          } catch {
            /* ignore */
          }
        })
        useEventBusStore.getState().removeGroupListeners(groupName)
      },
    } as FullStoreState<State, Actions>
  }
}

/**
 * Global singleton store. Register it on a module via
 * `createModule({ stores: [MyStore] })` — the name is written ONCE here, and
 * consumers read it through `Stores.<name>` exactly as before.
 */
export function defineStore<State extends object, Actions extends object>(
  name: string,
  config: StoreConfig<State, Actions>,
): StoreHandle<FullStoreState<State, Actions>> {
  const builder = makeBuilder(name, config)
  const store = (
    config.immer
      ? create<FullStoreState<State, Actions>>()(
          subscribeWithSelector(immerMiddleware(builder as any)),
        )
      : create<FullStoreState<State, Actions>>()(subscribeWithSelector(builder as any))
  ) as BoundStore<FullStoreState<State, Actions>>
  return { name, store }
}

// ============================================================================
// defineLocalStore — PRIVATE, per-component-instance stores.
//
// Same authoring model (state / actions / init / immer / $) as defineStore, but:
//   - NOT registered in Stores.X (each mount gets its own instance)
//   - `init` runs on MOUNT; every `on`/`watch` auto-unsubscribes on UNMOUNT
//     (a plain useEffect cleanup — no ref-count / delayed-destroy guessing).
//   - reactive reads keep the same shape: `const { a, b } = MyDef.use()`.
//
//   const Filter = defineLocalStore({
//     immer: true,
//     state: { query: '' },
//     actions: (set) => ({ setQuery: (q: string) => set(d => { d.query = q }) }),
//     init: ({ on }) => { on('sync:tags', () => {}) },  // torn down on unmount
//   })
//   function Panel() {
//     const s = Filter.use({ query: 'x' })   // fresh per mount
//     const { query } = s                     // reactive (same clean syntax)
//     return <input value={query} onChange={e => s.setQuery(e.target.value)} />
//   }
// ============================================================================

/** A per-instance store handle: reactive reads (`const {a}=s`), `s.$.a` snapshot,
 *  and `s.action()`. */
export type LocalStoreInstance<FullState> = Readonly<FullState & { $: FullState }>

export interface LocalStoreDef<FullState> {
  /** Instantiate a fresh store for THIS component (initial-state override
   *  merged in). Call it in render like any hook. */
  use: (initial?: Partial<FullState>) => LocalStoreInstance<FullState>
}

function createLocalProxy<S extends object>(
  api: StoreApi<S>,
): LocalStoreInstance<S> {
  return new Proxy({} as LocalStoreInstance<S>, {
    get: (_, prop) => {
      if (prop === '$' || prop === '__state') return api.getState()
      const value = (api.getState() as any)[prop]
      if (typeof value === 'function') return value
      // eslint-disable-next-line react-hooks/rules-of-hooks
      return useStore(api, useShallow((s: any) => s[prop]))
    },
  })
}

export function defineLocalStore<State extends object, Actions extends object>(
  config: StoreConfig<State, Actions>,
): LocalStoreDef<FullStoreState<State, Actions>> {
  // A distinct EventBus group per live instance so instances don't clobber each
  // other's listeners (defineStore's global variant can key by the store name;
  // locals can't).
  let counter = 0
  return {
    use: (initial) => {
      const ref = useRef<{
        api: StoreApi<FullStoreState<State, Actions>>
        proxy: LocalStoreInstance<FullStoreState<State, Actions>>
      } | null>(null)

      if (ref.current === null) {
        const group = `local:${counter++}`
        const merged: StoreConfig<State, Actions> = initial
          ? { ...config, state: { ...config.state, ...initial } as State }
          : config
        const builder = makeBuilder(group, merged)
        const api = (
          config.immer
            ? createStore<FullStoreState<State, Actions>>()(
                subscribeWithSelector(immerMiddleware(builder as any)),
              )
            : createStore<FullStoreState<State, Actions>>()(
                subscribeWithSelector(builder as any),
              )
        ) as StoreApi<FullStoreState<State, Actions>>
        ref.current = { api, proxy: createLocalProxy(api) }
      }

      const { api, proxy } = ref.current
      // init on mount, teardown on unmount — lifecycle rides the component.
      useEffect(() => {
        api.getState().__init__.__store__()
        return () => api.getState().__destroy__()
        // eslint-disable-next-line react-hooks/exhaustive-deps
      }, [])
      return proxy
    },
  }
}
