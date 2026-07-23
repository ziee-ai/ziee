/**
 * The single source of truth for how a conversation is LABELLED in the UI.
 *
 * Before this helper the fallback was copy-pasted at nine call sites with two
 * different literals ('Untitled Conversation' and 'Conversation'), so a
 * conversation could be named inconsistently depending on which surface you
 * looked at.
 *
 * Precedence: real `title` → `first_message_preview` → the placeholder.
 *
 * The middle rung exists because title generation deliberately leaves `title`
 * NULL rather than persisting the user's raw first message (a provider hiccup
 * must not become a permanent bad title). Without a fallback, a deployment
 * whose title provider is misconfigured renders every row as the SAME
 * "Untitled Conversation" string, which the user cannot tell apart.
 *
 * IMPORTANT: this is a DISPLAY label only. It must never be written back to the
 * `title` column — see `TitleEditor`, which edits the real field. Persisting the
 * preview would re-introduce exactly the raw-message-as-title behavior that was
 * deliberately removed.
 */

import { mathToPlainText } from '@/components/common/mathPlainText'

export const UNTITLED_CONVERSATION_LABEL = 'Untitled Conversation'

/** The fields this helper needs — structural, so it accepts any conversation-ish shape. */
export interface ConversationLabelSource {
  title?: string | null
  first_message_preview?: string | null
}

/**
 * Whitespace-only counts as absent: it renders as a blank row, which is strictly
 * worse than the placeholder. Mirrors the backend's `has_title` semantics.
 */
function firstNonBlank(...candidates: (string | null | undefined)[]): string | undefined {
  for (const candidate of candidates) {
    const trimmed = candidate?.trim()
    if (trimmed) return trimmed
  }
  return undefined
}

export function conversationDisplayLabel(
  conversation: ConversationLabelSource | null | undefined,
): string {
  const label = firstNonBlank(conversation?.title, conversation?.first_message_preview)
  // The preview rung is the user's RAW first message, so it carries whatever
  // markup they typed — including `\( … \)`, which no list surface can render.
  // Applied after the precedence choice so an empty result can still fall back.
  return firstNonBlank(label && mathToPlainText(label)) ?? UNTITLED_CONVERSATION_LABEL
}
