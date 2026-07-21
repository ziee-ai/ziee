import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { ScheduleLoopButton } from './components/ScheduleLoopButton'

/**
 * Schedule / Loop Extension (Group E — ITEM-18/20 + ITEM-24 FE)
 *
 * Adds a "Schedule or loop this chat" button to the composer toolbar. It opens a
 * merged dialog that creates a `scheduled_task` (prompt target) BOUND to the
 * current conversation — either a fixed Once/Recurring schedule, or a self-paced
 * "/loop" run with an optional goal-seeking "stop when…" completion condition.
 *
 * The backend is already end-to-end (scheduler module: `bound_conversation_id`,
 * `schedule_kind = 'self_paced'`, `completion_condition`); this is the in-chat
 * entry point over the existing `ScheduledTasks` store — no new store, no forked
 * schedule logic. Mirrors the voice extension's `toolbar_actions` slot precedent
 * (DEC-41: toolbar button is the sole entry).
 */
const scheduleExtension: ChatExtension = createExtension({
  name: 'schedule',
  description: 'Schedule or loop a task bound to the current conversation',
  priority: 84,

  // Sits just left of the voice mic (order 85) in the composer toolbar.
  slots: {
    toolbar_actions: { component: ScheduleLoopButton, order: 84 },
  },
})

export default scheduleExtension
