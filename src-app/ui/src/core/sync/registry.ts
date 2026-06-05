import type { SyncAction, SyncEntity } from '@/api-client/types'
import type { AppEvents } from '@/core/events'
import { Stores } from '@/core/stores'
import type { SyncEntityEvent } from './types'

/**
 * How a module reacts to a remote change to its entity.
 * - `onEvent` applies one change (the per-surface policy lives here:
 *   small-list reload / paginated id-aware update / infinite-scroll
 *   prepend-on-create).
 * - `onResync` (optional) reloads the surface wholesale; called on every
 *   (re)connect to cover events missed while the stream was down
 *   (best-effort durability).
 */
export interface SyncRegistration {
  onEvent: (action: SyncAction, id: string) => void
  onResync?: () => void
}

const registrations = new Map<SyncEntity, SyncRegistration>()

/**
 * The full set of syncable entities, for the boot-time coverage check.
 * Hand-maintained but typed — `satisfies readonly SyncEntity[]` rejects a
 * typo'd entity name. Grow this alongside the Rust `SyncEntity` enum.
 */
const ALL_SYNC_ENTITIES = [
  'project',
  'assistant',
  'mcp_server',
  'memory',
  'memory_settings',
  'api_key',
  'llm_provider',
  'llm_model',
  'user_llm_provider',
] as const satisfies readonly SyncEntity[]

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
  Stores.EventBus.on(
    eventType,
    ((event: SyncEntityEvent) => {
      registration.onEvent(event.data.action, event.data.id)
    }) as never,
    'sync',
  )
}

/** Reload every synced surface — called on each SSE (re)connect. */
export function resyncAll(): void {
  for (const registration of registrations.values()) {
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
  const missing = ALL_SYNC_ENTITIES.filter(e => !registrations.has(e))
  if (missing.length === 0) return
  const message = `[sync] no refetch handler registered for entities: ${missing.join(', ')}`
  if (import.meta.env.DEV) {
    throw new Error(message)
  }
  console.error(message)
}
