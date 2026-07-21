import { ApiClient } from '@/api-client'

/**
 * Agent / background notification kinds — the results a user gets back when a
 * BACKGROUND sub-agent group or a SCHEDULED loop they launched returns. This is
 * the classifier behind the "Background results" inbox view (ITEM-26 / DEC-65:
 * a FE composition over the existing notification inbox).
 *
 * Why a typed constant rather than a pure registry read: the backend kind
 * registry (`GET /api/notifications/kinds`, `NotificationKindDescriptor`) carries
 * NO "category" field, and the scheduler kinds are PRODUCED but not (yet)
 * registered in the `NOTIFICATION_KINDS` slice — so a pure registry read can
 * neither classify agent/background vs other kinds nor even enumerate the
 * scheduler kinds. This constant is therefore the canonical set. We STILL
 * consult the registry (see `resolveAgentInboxKinds`) to UNION in any advertised
 * kind whose key matches the background/agent naming pattern, so a future backend
 * agent kind that IS registered surfaces here with zero FE edit.
 *
 * Payload conventions the go-to-result affordance relies on:
 *  - `background_run_result` → `{ workflow_run_id, conversation_id }`
 *  - `scheduled_task_result` → `{ scheduled_task_id, workflow_run_id?, conversation_id? }`
 *  - `scheduled_task_failed` → `{ scheduled_task_id }`
 */
export const AGENT_INBOX_KINDS = [
  'background_run_result',
  'scheduled_task_result',
  'scheduled_task_failed',
] as const

/**
 * Matches background/agent kind keys the registry may advertise beyond the
 * constant above (forward-compat: a newly registered agent kind auto-appears).
 */
const AGENT_KIND_PATTERN = /^background_|_run_result$|^scheduled_task_|^agent_/

/**
 * The effective agent/background kind set for the current deployment: the typed
 * constant UNION any registry-advertised kind whose key matches the
 * agent/background naming pattern. Best-effort — a registry fetch failure
 * degrades to the constant (the constant is the source of truth, the registry is
 * only augmentation).
 */
export async function resolveAgentInboxKinds(): Promise<Set<string>> {
  const set = new Set<string>(AGENT_INBOX_KINDS)
  try {
    const kinds = await ApiClient.Notification.kinds()
    for (const d of kinds) {
      if (AGENT_KIND_PATTERN.test(d.kind)) set.add(d.kind)
    }
  } catch {
    /* registry augmentation is best-effort; the constant already covers the
       known agent/background kinds. */
  }
  return set
}
