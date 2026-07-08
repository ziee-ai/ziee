import type { MessageWithContent } from '@/api-client/types'

/**
 * Pure helpers for the lazy-loaded message WINDOW (ITEM-6 / ITEM-7).
 *
 * The chat store holds the loaded slice of the active branch as a
 * `Map<id, MessageWithContent>` whose INSERTION ORDER is the render order
 * (`MessageList` renders `Array.from(messages.values())`). These helpers keep
 * that order correct as pages are prepended (scroll-up), appended (scroll-down),
 * or the tail is merged after a streamed turn — without dropping already-loaded
 * messages or duplicating overlapping ids. Extracted as pure functions so the
 * ordering invariants are unit-testable (TEST-3) independent of the store.
 */

/** Build an ordered Map from a chronological message array. */
export function toOrderedMap(
  messages: MessageWithContent[],
): Map<string, MessageWithContent> {
  const next = new Map<string, MessageWithContent>()
  for (const msg of messages) next.set(msg.id, msg)
  return next
}

/**
 * Prepend an OLDER page in front of the existing window. Older-page entries
 * keep their (older) order; any id already present in `existing` is skipped so
 * the existing entry keeps its position (no duplicate, no reorder).
 */
export function prependWindow(
  existing: Map<string, MessageWithContent>,
  olderPage: MessageWithContent[],
): Map<string, MessageWithContent> {
  const next = new Map<string, MessageWithContent>()
  for (const msg of olderPage) {
    if (!existing.has(msg.id)) next.set(msg.id, msg)
  }
  for (const [id, msg] of existing) next.set(id, msg)
  return next
}

/**
 * Append a NEWER page after the existing window (scroll-down after an `around`
 * jump). Newer-page entries that already exist update in place (keeping their
 * position); genuinely-new ones append in page order.
 */
export function appendWindow(
  existing: Map<string, MessageWithContent>,
  newerPage: MessageWithContent[],
): Map<string, MessageWithContent> {
  const next = new Map(existing)
  for (const msg of newerPage) next.set(msg.id, msg)
  return next
}

/**
 * Merge a TAIL (newest) page into the existing window after a streamed turn or
 * a cross-device change. Overlapping ids update in place; genuinely-new tail
 * messages append at the end in page order. Crucially it NEVER drops
 * already-loaded older entries — a user who scrolled up + loaded older pages
 * keeps them while the new turn still appears at the bottom.
 *
 * (Same mechanic as {@link appendWindow}; named separately because the intent —
 * reconciling the tail after `complete` — differs from paging newer.)
 */
export function mergeTailWindow(
  existing: Map<string, MessageWithContent>,
  tailPage: MessageWithContent[],
): Map<string, MessageWithContent> {
  return appendWindow(existing, tailPage)
}

/** First (oldest-loaded) message id, or null when empty. */
export function firstMessageId(
  messages: Map<string, MessageWithContent>,
): string | null {
  const it = messages.keys().next()
  return it.done ? null : it.value
}

/** Last (newest-loaded) message id, or null when empty. */
export function lastMessageId(
  messages: Map<string, MessageWithContent>,
): string | null {
  let last: string | null = null
  for (const id of messages.keys()) last = id
  return last
}
