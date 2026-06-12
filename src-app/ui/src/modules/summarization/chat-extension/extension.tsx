import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { SummaryBoundaryMarker } from '@/modules/summarization/chat-extension/components/SummaryBoundaryMarker'
import { SummarizationStatusPill } from '@/modules/summarization/chat-extension/components/SummarizationStatusPill'

// Summarization Extension (frontend chat-extension shim).
//
// The actual apply-summary / refresh-summary backend lives in
// modules/summarization/{chat_extension,engine} on the server side.
// This extension registers two slot components:
//   - `toolbar_status`: SummarizationStatusPill (per-conversation
//     mode + drives the read-model load for the in-thread marker).
//   - `message_footer`: SummaryBoundaryMarker (renders on the message
//     at `summary.summarized_up_to_id`, expandable).
//
// Auto-discovered by chat/extensions/index.ts via the
// import.meta.glob pattern over `../../*/chat-extension/extension.tsx`.
//
// No composeRequestFields: the backend summarization bridge reads the
// per-conversation mode from `conversation_summarization_settings`
// when assembling the prompt; the frontend pill writes via
// PUT /api/conversations/{id}/summarization-mode.
const summarizationExtension: ChatExtension = createExtension({
  name: 'summarization',
  description:
    'Per-conversation summarization override pill + in-thread summary boundary marker',
  // Render after the memory pill (order 30) so the two appear in a
  // predictable left-to-right reading order.
  priority: 90,

  slots: {
    toolbar_status: { component: SummarizationStatusPill, order: 40 },
    message_footer: { component: SummaryBoundaryMarker, order: 10 },
  },
})

export default summarizationExtension
