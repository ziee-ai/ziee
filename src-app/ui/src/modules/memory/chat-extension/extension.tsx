import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { MemoryStatusPill } from '@/modules/memory/chat-extension/components/MemoryStatusPill'

// Memory Extension (frontend chat-extension shim).
//
// The actual retrieval / extraction backend lives in
// modules/memory/{chat_extension,engine} on the server side. This
// extension is purely a UI hook: it registers a toolbar_status slot
// component (the per-conversation memory-mode pill, Plan §7 Phase 5).
// Auto-discovered by chat/extensions/index.ts via the
// import.meta.glob pattern over `../../*/chat-extension/extension.tsx`.
//
// No composeRequestFields needed — the backend memory bridge reads
// the per-conversation mode from `conversation_memory_settings`
// (migration 76) when assembling the prompt; the frontend pill
// writes via PUT /api/conversations/{id}/memory-mode.
const memoryExtension: ChatExtension = createExtension({
  name: 'memory',
  description: 'Per-conversation memory retrieval override pill',
  // Render late so the pill appears after the assistant / MCP chips
  // (existing chips use order 10 + 20; we use 30).
  priority: 90,

  slots: {
    toolbar_status: { component: MemoryStatusPill, order: 30 },
  },

  // After the stream completes, refresh the Memories store so any
  // auto-extracted memories from the backend's after_llm_call hook
  // become visible on the Memories page without a manual reload.
  // Best-effort: if extraction hasn't finished server-side, the
  // sync:memory event subscription handles eventual consistency.
  afterStreamComplete: async (_message) => {
    // Dynamic import: this chat-extension is EAGERLY discovered, so a static
    // import would drag the Memories store shell into the entry chunk. Lazy
    // import keeps it out (loads only when a stream actually completes).
    const { Memories } = await import('@/modules/memory/stores/memories')
    Memories.load()
    return {}
  },
})

export default memoryExtension
