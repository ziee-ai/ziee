/**
 * chatDrafts — localStorage-backed composer drafts (ITEM-7).
 *
 * Unsent composer text is persisted per conversation so it survives navigating
 * away and back. The new-chat page (no conversation yet) uses the `new` bucket;
 * a successful send from there creates a conversation and clears `new`.
 *
 * Client-only (localStorage), matching the "persist across navigation" scope —
 * not a cross-device/synced surface (see DEC-8). All access is guarded so a
 * disabled/þrowing storage (private mode, quota) degrades to a no-op instead of
 * breaking the composer.
 */

const PREFIX = 'ziee:chat-draft:'
/** Bucket for the new-chat composer (no conversation id yet). */
export const NEW_DRAFT_KEY = 'new'

function storageKey(key: string): string {
  return `${PREFIX}${key}`
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
 * Clear the draft for a conversation key AND the `new` bucket. Clearing `new`
 * on every send covers the new-chat→created transition: the text was saved
 * under `new`, but by the time the message is sent the active key has become
 * the freshly-created conversation id.
 */
export function clearDraft(key: string): void {
  try {
    localStorage.removeItem(storageKey(key))
    if (key !== NEW_DRAFT_KEY) {
      localStorage.removeItem(storageKey(NEW_DRAFT_KEY))
    }
  } catch {
    // ignore
  }
}
