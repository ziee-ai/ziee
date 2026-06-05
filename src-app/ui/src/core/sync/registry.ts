import type { SyncAction, SyncEntity } from '@/api-client/types'
import type { AppEvents } from '@/core/events'
import { useEventBusStore } from '@/core/events/store'
import type { PermissionExpr } from '@/core/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { SyncEntityEvent } from './types'

/**
 * How a module reacts to a remote change to its entity.
 * - `onEvent` applies one change (the per-surface policy lives here:
 *   small-list reload / paginated id-aware update / infinite-scroll
 *   prepend-on-create).
 * - `onResync` (optional) reloads the surface wholesale; called on every
 *   (re)connect to cover events missed while the stream was down
 *   (best-effort durability).
 * - `requiredPermission` (optional) — for entities whose refetch hits a
 *   permission-gated endpoint, the perm the user must hold. `resyncAll`
 *   fires for ALL handlers regardless of the server audience, so without
 *   this a non-admin's reconnect would call admin-only loads → 403 (and the
 *   `no-403` E2E gate fails). Both `onEvent` and `onResync` are skipped when
 *   the user lacks it (defense-in-depth; `onEvent` is normally already
 *   server-gated by the audience routing).
 */
export interface SyncRegistration {
  onEvent: (action: SyncAction, id: string) => void
  onResync?: () => void
  requiredPermission?: PermissionExpr
}

const registrations = new Map<SyncEntity, SyncRegistration>()

/**
 * Coverage map: EVERY `SyncEntity` must appear here. Because the type is
 * `Record<SyncEntity, ...>`, a newly-generated Rust entity that is missing
 * from this map is a COMPILE error — it can't ship without a frontend
 * decision (the plan's "every SyncEntity has a handler", enforced statically).
 *   - `'handled'`     → a module calls `registerSync` for it (asserted at boot).
 *   - `'backend-only'` → it emits server-side but has no live frontend surface
 *                        to invalidate. (None today — every entity is handled.)
 */
const ENTITY_COVERAGE: Record<SyncEntity, 'handled' | 'backend-only'> = {
  project: 'handled',
  assistant: 'handled',
  mcp_server: 'handled',
  memory: 'handled',
  memory_settings: 'handled',
  profile: 'handled',
  api_key: 'handled',
  llm_provider: 'handled',
  llm_model: 'handled',
  user_llm_provider: 'handled',
  group: 'handled',
  user: 'handled',
  assistant_template: 'handled',
  mcp_server_system: 'handled',
  user_mcp_server: 'handled',
  llm_repository: 'handled',
  runtime_version: 'handled',
  memory_admin_settings: 'handled',
  code_sandbox_settings: 'handled',
  hub_settings: 'handled',
  session: 'handled',
}

/**
 * Register a module's reaction to remote changes for one entity. Sugar
 * over `EventBus.on('sync:<entity>')` — the EventBus routes by type so the
 * handler only ever fires for this entity. Called once at module load.
 */
export function registerSync(
  entity: SyncEntity,
  registration: SyncRegistration,
): void {
  registrations.set(entity, registration)
  const eventType = `sync:${entity}` as keyof AppEvents
  useEventBusStore.getState().on(
    eventType,
    ((event: SyncEntityEvent) => {
      if (
        registration.requiredPermission &&
        !hasPermissionNow(registration.requiredPermission)
      ) {
        return
      }
      registration.onEvent(event.data.action, event.data.id)
    }) as never,
    'sync',
  )
}

/** Reload every synced surface — called on each SSE (re)connect. */
export function resyncAll(): void {
  for (const registration of registrations.values()) {
    // Skip handlers whose refetch the user isn't permitted to make (a
    // non-admin reconnecting must not fire admin-only loads → 403).
    if (
      registration.requiredPermission &&
      !hasPermissionNow(registration.requiredPermission)
    ) {
      continue
    }
    try {
      registration.onResync?.()
    } catch (error) {
      console.error('[sync] resync handler failed:', error)
    }
  }
}

/**
 * Fail loudly in dev if any syncable entity lacks a registered handler
 * (a module forgot to call `registerSync`). Run once after modules load.
 */
export function assertSyncCoverage(): void {
  const missing = (Object.keys(ENTITY_COVERAGE) as SyncEntity[]).filter(
    e => ENTITY_COVERAGE[e] === 'handled' && !registrations.has(e),
  )
  if (missing.length === 0) return
  const message = `[sync] entities marked 'handled' but with no registered handler: ${missing.join(', ')}`
  if (import.meta.env.DEV) {
    throw new Error(message)
  }
  console.error(message)
}
