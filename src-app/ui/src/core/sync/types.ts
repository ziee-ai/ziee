import type { SyncAction, SyncEntity } from '@/api-client/types'
import type { BaseEvent } from '@/core/events'

/**
 * A remote change notification for one entity, re-emitted onto the client
 * EventBus as a per-entity `sync:<entity>` event. Carries only the action
 * + id (notify-and-refetch) — the registered handler refetches the entity
 * itself via the existing permission-checked REST endpoint.
 *
 * Per-entity event names (not one global `sync` event) let the EventBus
 * route by type: a store subscribes to exactly its own `sync:<entity>` and
 * its handler never runs for other entities.
 */
export interface SyncEntityEvent<E extends SyncEntity = SyncEntity>
  extends BaseEvent {
  type: `sync:${E}`
  data: { action: SyncAction; id: string }
}

/**
 * The `sync:<entity>` event map, DERIVED from the generated `SyncEntity`
 * union via a key-remapped mapped type. The Rust `SyncEntity` enum is the
 * single source of truth: it's generated into `@/api-client/types` by the
 * OpenAPI regen, so a new entity flows into this map automatically — there is
 * NO hand-maintained list to keep in lockstep with the backend.
 */
type SyncEntityEvents = {
  [E in SyncEntity as `sync:${E}`]: SyncEntityEvent<E>
}

declare module '@/core/events' {
  interface AppEvents extends SyncEntityEvents {}
}
