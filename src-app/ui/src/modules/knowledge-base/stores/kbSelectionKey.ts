/**
 * Per-conversation KB composer selection keying (ITEM-46).
 *
 * The KB composer selection was a single flat set tied to one global
 * `currentConversationId`, so two split panes on different conversations showed +
 * mutated the SAME selection (last-loader wins). It is now keyed per conversation
 * (or a pending key for a not-yet-created new chat), mirroring `McpComposer`'s
 * per-conversation Maps. These pure helpers are the key logic, extracted so the
 * per-conversation isolation is unit-testable without importing the api-client-laden
 * store.
 */

/** New-chat pending buffer key (before a conversation id is minted). */
export const PENDING_KB_KEY = '__pending__'

/** Resolve a conversation id (null → the pending new-chat key) to its map key. */
export const kbKey = (conversationId: string | null | undefined): string =>
  conversationId || PENDING_KB_KEY

/** A conversation's selected KB ids from the per-conversation map (empty if none). */
export function selectedKbIdsFor(
  map: ReadonlyMap<string, ReadonlySet<string>>,
  conversationId: string | null | undefined,
): string[] {
  return Array.from(map.get(kbKey(conversationId)) ?? [])
}
