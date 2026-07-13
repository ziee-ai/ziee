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

// Whether to render the inline "empty completion" notice for a message.
//
// Only for a FINALISED assistant turn (`!isStreaming`, so a live turn that
// momentarily has only a thinking block never flashes the notice) that produced
// no visible answer — AND was not `interrupted` (a user-cancelled / stream-
// errored / aborted turn is a partial, not a genuine empty completion, so the
// notice would misattribute the cause; the caller passes the store's
// per-turn interruption signal) — AND is not `finalizing` (the sub-second
// streaming→persisted handoff window: `isStreaming` has flipped false but the
// persisted tail may not be swapped in yet, so a transient empty assistant frame
// must not flash the notice; the caller passes the store's `finalizingTurn`).
export function shouldShowEmptyCompletionNotice(opts: {
  isUser: boolean
  isStreaming: boolean
  interrupted: boolean
  finalizing: boolean
  message: MessageWithContent
}): boolean {
  const { isUser, isStreaming, interrupted, finalizing, message } = opts
  return (
    !isUser &&
    !isStreaming &&
    !interrupted &&
    !finalizing &&
    !hasVisibleAnswer(message)
  )
}
