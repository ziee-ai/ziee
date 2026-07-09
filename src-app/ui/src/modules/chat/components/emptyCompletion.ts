import type {
  MessageContent,
  MessageContentDataText,
  MessageWithContent,
} from '@/api-client/types'

// Does a single content block count as a **user-visible answer**?
//
// Mirrors the backend `is_visible_answer` (server `streaming.rs`): a `thinking`
// block and an empty/whitespace `text` block do NOT count — they are what make a
// turn *appear* to hang with nothing shown. Every other content type
// (`tool_use` / `tool_result` / `image` / `file_attachment` /
// `elicitation_request` / …) is a visible answer.
export function isVisibleAnswerBlock(block: MessageContent): boolean {
  switch (block.content_type) {
    case 'thinking':
      return false
    case 'text':
      return ((block.content as MessageContentDataText).text ?? '').trim().length > 0
    default:
      return true
  }
}

// True when an assistant message produced a user-visible answer. A finalised
// assistant turn for which this is false is the "empty completion" case (only
// reasoning, or nothing) — the caller surfaces an inline notice instead of
// rendering nothing.
export function hasVisibleAnswer(message: MessageWithContent): boolean {
  return (message.contents ?? []).some(isVisibleAnswerBlock)
}
