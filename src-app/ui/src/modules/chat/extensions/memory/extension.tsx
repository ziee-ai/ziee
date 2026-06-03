import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { MemoryStatusPill } from '@/modules/chat/extensions/memory/components/MemoryStatusPill'

// Memory Extension (frontend chat-extension shim).
//
// The actual retrieval / extraction / MCP backend lives in
// modules/memory + modules/chat/extensions/memory on the server side.
// This extension is purely a UI hook: it registers a toolbar_status
// slot component (the per-conversation memory-mode pill, Plan §7
// Phase 5). Auto-discovered by chat/extensions/index.ts via the
// import.meta.glob pattern over ./STAR/extension.tsx.
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
})

export default memoryExtension
