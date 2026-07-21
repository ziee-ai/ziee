import type { McpComposerSet, McpComposerGet } from '../state'
import type { McpToolProgressState } from '../state'

/**
 * Attach progress to the running ('started') tool call(s) for a server.
 * `notifications/progress` carry server + message_id but not tool_use_id,
 * so we correlate to the in-flight call(s) from that server (typically
 * the single running execute_command download).
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  server: string,
  messageId: string | undefined,
  progress: McpToolProgressState,
) => {
  set(state => {
    for (const [id, call] of state.toolCalls) {
      // Match server AND the owning streaming message (ITEM-33/48), so a
      // progress event only updates the pane whose message spawned the call.
      // BUT a call is stamped with `streamingMessage?.id`, which may be a
      // synthetic client placeholder (`streaming-<ts>`, see Chat.store
      // placeholderId) that will NEVER equal the real server message_id a
      // progress event carries — so only a REAL (non-placeholder) call id is a
      // usable discriminator. When either side lacks a usable id, fall back to
      // server-only (the pre-split behaviour) so the progress bar never
      // silently stalls; the message_id refinement then only kicks in to avoid
      // cross-pane cross-talk when a real id IS available on both sides.
      const callMsgId = call.message_id
      const usableCallId = !!callMsgId && !callMsgId.startsWith('streaming-')
      const messageMatch = !messageId || !usableCallId || callMsgId === messageId
      if (call.server === server && messageMatch && call.status === 'started') {
        state.toolCalls.set(id, { ...call, progress })
      }
    }
  })
}
