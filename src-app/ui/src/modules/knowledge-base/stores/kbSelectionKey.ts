/**
 * Per-conversation KB composer selection keying (ITEM-46, ITEM-51).
 *
 * The KB composer selection was a single flat set tied to one global
 * `currentConversationId`, so two split panes on different conversations showed +
 * mutated the SAME selection (last-loader wins). It is now keyed per conversation
 * (or a per-PANE pending key for a not-yet-created new chat), mirroring
 * `McpComposer`'s per-conversation Maps. These pure helpers are the key logic,
 * extracted so the per-conversation/per-pane isolation is unit-testable without
 * importing the api-client-laden store.
 *
 * COMMITTED state keys by conversation id (unique once minted). PENDING (new-chat,
 * pre-mint) state keys by PANE (ITEM-51): two split panes each composing a NEW
 * chat must NOT share one pending buffer. Single-pane (a null paneId) keeps the
 * bare `__pending__` key, so the single-pane behaviour is byte-identical.
 */

/** New-chat pending buffer base key (single-pane / no pane). */
export const PENDING_KB_KEY = '__pending__'

/** Per-pane pending buffer key: `__pending__:<paneId>`, or the bare key when
 *  there is no pane (single-pane), so two new-chat split panes don't collide. */
export const pendingKbKey = (paneId?: string | null): string =>
  paneId ? `${PENDING_KB_KEY}:${paneId}` : PENDING_KB_KEY

/** Resolve a conversation id (null → the pane's pending new-chat key) to its map key. */
export const kbKey = (
  conversationId: string | null | undefined,
  paneId?: string | null,
): string => conversationId || pendingKbKey(paneId)

/** A conversation's (or pane's pending) selected KB ids from the map (empty if none). */
export function selectedKbIdsFor(
  map: ReadonlyMap<string, ReadonlySet<string>>,
  conversationId: string | null | undefined,
  paneId?: string | null,
): string[] {
  return Array.from(map.get(kbKey(conversationId, paneId)) ?? [])
}
