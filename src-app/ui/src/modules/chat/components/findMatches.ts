import type { MessageWithContent } from '@/api-client/types'

/**
 * findMatches — in-conversation search (ITEM-1).
 *
 * Pure, client-side match over the already-loaded messages: returns the ids of
 * messages whose TEXT content contains `query` (case-insensitive substring), in
 * the given display order. Non-text blocks (files, tool calls) are ignored. A
 * blank/whitespace query matches nothing.
 *
 * Message-level matching (not substring offsets): the find bar jumps to and
 * highlights whole messages (DEC-4), so the caller only needs the ordered id
 * list + its length for the "X of Y" readout.
 */
export function findMatches(
  messages: MessageWithContent[],
  query: string,
): string[] {
  const needle = query.trim().toLowerCase()
  if (needle.length === 0) return []

  const matches: string[] = []
  for (const message of messages) {
    if (messageText(message).toLowerCase().includes(needle)) {
      matches.push(message.id)
    }
  }
  return matches
}

/** Concatenate a message's `text` content blocks into a single searchable string. */
export function messageText(message: MessageWithContent): string {
  if (!message.contents) return ''
  let out = ''
  for (const content of message.contents) {
    if (content.content_type === 'text') {
      const text = (content.content as { text?: string } | null)?.text
      if (typeof text === 'string') out += (out ? '\n' : '') + text
    }
  }
  return out
}
