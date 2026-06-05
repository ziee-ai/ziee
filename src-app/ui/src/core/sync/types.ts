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

declare module '@/core/events' {
  interface AppEvents {
    // ADD a `sync:<entity>` key here when wiring a new domain. The Rust
    // `SyncEntity` enum is the source of truth (generated into
    // `SyncEntity` above); this list grows alongside it.
    'sync:project': SyncEntityEvent<'project'>
    'sync:assistant': SyncEntityEvent<'assistant'>
    'sync:mcp_server': SyncEntityEvent<'mcp_server'>
    'sync:memory': SyncEntityEvent<'memory'>
    'sync:memory_settings': SyncEntityEvent<'memory_settings'>
    'sync:api_key': SyncEntityEvent<'api_key'>
  }
}
