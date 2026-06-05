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
    // `SyncEntity` enum is the source of truth (generated into the
    // `SyncEntity` type in `@/api-client/types`, imported above); this list
    // grows alongside it.
    'sync:project': SyncEntityEvent<'project'>
    'sync:assistant': SyncEntityEvent<'assistant'>
    'sync:mcp_server': SyncEntityEvent<'mcp_server'>
    'sync:memory': SyncEntityEvent<'memory'>
    'sync:memory_settings': SyncEntityEvent<'memory_settings'>
    'sync:api_key': SyncEntityEvent<'api_key'>
    'sync:llm_provider': SyncEntityEvent<'llm_provider'>
    'sync:llm_model': SyncEntityEvent<'llm_model'>
    'sync:user_llm_provider': SyncEntityEvent<'user_llm_provider'>
    'sync:group': SyncEntityEvent<'group'>
    'sync:user': SyncEntityEvent<'user'>
    'sync:assistant_template': SyncEntityEvent<'assistant_template'>
    'sync:mcp_server_system': SyncEntityEvent<'mcp_server_system'>
    'sync:llm_repository': SyncEntityEvent<'llm_repository'>
    'sync:runtime_version': SyncEntityEvent<'runtime_version'>
    'sync:runtime_settings': SyncEntityEvent<'runtime_settings'>
    'sync:memory_admin_settings': SyncEntityEvent<'memory_admin_settings'>
    'sync:code_sandbox_settings': SyncEntityEvent<'code_sandbox_settings'>
    'sync:session': SyncEntityEvent<'session'>
    'sync:profile': SyncEntityEvent<'profile'>
    'sync:hub_settings': SyncEntityEvent<'hub_settings'>
    'sync:user_mcp_server': SyncEntityEvent<'user_mcp_server'>
  }
}
