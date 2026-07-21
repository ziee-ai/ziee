import { useContext } from 'react'
import { type StoreApi, useStore } from 'zustand'
import { Chat as ChatPrimary } from '@/modules/chat/core/stores/chat'
import { useSplitViewStore } from '@/modules/chat/core/stores/splitView'
import { PaneApiContext } from '@/modules/chat/core/pane/paneApiContext'

/**
 * `ChatStore` focused-pane bridge (ITEM-9 / DEC-5).
 *
 * `ChatStore` is registered as THIS object (not the primary store directly),
 * so every existing `ChatStore` consumer transparently reads/acts on whichever
 * pane is FOCUSED — defaulting to the primary pane (`ChatPrimary.store`, pane 0) when no
 * split pane is focused. In single-pane mode `paneRegistry` is empty and
 * `focusedPaneId` is null, so the bridge forwards to the primary and behaviour is
 * byte-identical to before the split existed.
 *
 * The module system wraps this with `createStoreProxy` at registration, so
 * consumers get the usual 4-mode proxy ($ snapshot / actions / nested / reactive)
 * — the proxy's `getState`/`setState`/hook-call all land on `focusedApi()`.
 *
 * (Reactive re-render across a focus CHANGE is a 2-pane concern handled by the
 * pane subtree using `useChatPane()`; single-pane focus is stable.)
 */

/** A live per-pane chat store instance, registered by `ChatPaneProvider`. */
export interface ChatPaneHandle {
  paneId: string
  api: StoreApi<ReturnType<typeof ChatPrimary.store.getState>>
}

/** paneId → its live store. Empty in single-pane mode. */
export const paneRegistry = new Map<string, ChatPaneHandle>()

export function registerPane(handle: ChatPaneHandle): void {
  paneRegistry.set(handle.paneId, handle)
}
export function unregisterPane(paneId: string): void {
  paneRegistry.delete(paneId)
}

/** The primary pane's api (pane 0) — the default bridge target. */
const primaryApi = (): StoreApi<ReturnType<typeof ChatPrimary.store.getState>> =>
  ChatPrimary.store as unknown as StoreApi<ReturnType<typeof ChatPrimary.store.getState>>

/** Resolve the StoreApi the bridge currently forwards to. */
function focusedApi(): StoreApi<ReturnType<typeof ChatPrimary.store.getState>> {
  const focusedId = useSplitViewStore.getState().focusedPaneId
  const handle = focusedId ? paneRegistry.get(focusedId) : undefined
  return handle?.api ?? primaryApi()
}

// A UseBoundStore-shaped facade: callable as a hook (reactive read via the
// module proxy's path 4) with getState/setState/subscribe forwarding.
//
// The hook body runs during a component render (createStoreProxy path 4), so it
// reads `PaneApiContext`: inside a pane subtree it forwards the reactive read to
// THAT pane's store; outside any pane it falls back to the focused/primary pane.
// This is what lets ~40 existing `ChatStore.<field>` reactive consumers stay
// pane-correct in split mode without being rewritten.
const bridge = ((selector: (s: unknown) => unknown) => {
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const paneApi = useContext(PaneApiContext)
  const api = (paneApi ?? focusedApi()) as StoreApi<unknown>
  // eslint-disable-next-line react-hooks/rules-of-hooks
  return useStore(api, selector)
}) as unknown as typeof ChatPrimary.store

bridge.getState = () => focusedApi().getState()
bridge.getInitialState = () => focusedApi().getInitialState()
bridge.setState = ((...args: unknown[]) =>
  (focusedApi().setState as (...a: unknown[]) => void)(...args)) as typeof ChatPrimary.store.setState
bridge.subscribe = ((...args: unknown[]) =>
  (focusedApi().subscribe as (...a: unknown[]) => () => void)(
    ...args,
  )) as typeof ChatPrimary.store.subscribe

/** The registered `ChatStore` (forwards to the focused pane). */
export const chatBridge = bridge

// Direct handle: the focused-pane bridge proxy (was `ChatStore`). `import
// { Chat }` + `Chat.field` forwards to the focused pane, identical to before.
import { createStoreProxy as _createStoreProxy } from '@ziee/framework/stores'
import type { StoreProxy as _StoreProxy } from '@ziee/framework/stores'
import type { ChatExtensionStores as _ChatExt } from '@/modules/chat/types'
export const Chat = _createStoreProxy(chatBridge) as _StoreProxy<
  ReturnType<typeof ChatPrimary.store.getState> & _ChatExt
>
