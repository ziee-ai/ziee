import { createContext } from 'react'
import type { StoreApi } from 'zustand'
import type { Chat } from '@/modules/chat/core/stores/chat'

/**
 * The chat `StoreApi` of the pane a subtree belongs to (ITEM-10 / DEC-5).
 *
 * `ChatPaneProvider` provides its per-pane store's raw api here; the `ChatStore`
 * bridge reads it during a reactive render so that EVERY existing
 * `ChatStore.<field>` reactive read inside a pane subtree resolves to THAT
 * pane — no per-component migration. It is `null` outside any pane (single-pane),
 * where the bridge falls back to the focused/primary pane and behaviour is
 * byte-identical.
 *
 * (Handler-time reads/actions — `ChatStore.$.x` / `ChatStore.doThing()` — run
 * outside render and cannot read context; they route to the FOCUSED pane, which
 * is correct because interacting with a pane focuses it first. Pane-owned
 * imperative logic that must not depend on focus uses the pane store directly via
 * `useChatPane()`.)
 */
export type ChatStoreApi = StoreApi<ReturnType<typeof Chat.store.getState>>

export const PaneApiContext = createContext<ChatStoreApi | null>(null)
