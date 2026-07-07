/**
 * chatDrafts — localStorage-backed composer drafts (ITEM-7).
 *
 * Unsent composer text is persisted per conversation so it survives navigating
 * away and back. The new-chat page (no conversation yet) uses the `new` bucket;
 * a successful send from there creates a conversation and clears `new`.
 *
 * Client-only (localStorage), matching the "persist across navigation" scope —
 * not a cross-device/synced surface (see DEC-8). All access is guarded so a
 * disabled/throwing storage (private mode, quota) degrades to a no-op instead of
 * breaking the composer.
 */

const PREFIX = 'ziee:chat-draft:'
/** Bucket for the new-chat composer (no conversation id yet). */
export const NEW_DRAFT_KEY = 'new'

function storageKey(key: string): string {
  return `${PREFIX}${key}`
}

/**
 * Build the per-conversation draft key, NAMESPACED BY USER ID. Namespacing is a
 * security requirement on shared browsers: without it, the fixed `new` bucket
 * would let the next user who logs in see the previous user's unsent new-chat
 * draft (localStorage survives logout). A different user id yields a different
 * key, so drafts are never cross-read. `anon` is a defensive fallback (the
 * composer is only reachable while authenticated).
 */
export function makeDraftKey(
  userId: string | null | undefined,
  conversationId: string | null | undefined,
): string {
  return `${userId ?? 'anon'}:${conversationId ?? NEW_DRAFT_KEY}`
}

/** Read the saved draft for a conversation key (or `''` if none / unavailable). */
export function getDraft(key: string): string {
  try {
    return localStorage.getItem(storageKey(key)) ?? ''
  } catch {
    return ''
  }
}

/**
 * Save (or, for empty/whitespace text, remove) the draft for a conversation
 * key. Empty drafts are removed so a cleared composer doesn't leave a stale key.
 */
export function setDraft(key: string, text: string): void {
  try {
    if (text.trim().length === 0) {
      localStorage.removeItem(storageKey(key))
    } else {
      localStorage.setItem(storageKey(key), text)
    }
  } catch {
    // ignore storage failures (private mode / quota) — draft persistence is
    // best-effort.
  }
}

/**
 * Clear the draft for exactly ONE conversation key. Callers that send from the
 * new-chat page must pass `NEW_DRAFT_KEY` (the key the text was authored under)
 * — captured BEFORE the conversation is created — so a send never wipes an
 * unrelated conversation's (or a separate new-chat) draft.
 */
export function clearDraft(key: string): void {
  try {
    localStorage.removeItem(storageKey(key))
  } catch {
    // ignore
  }
}
