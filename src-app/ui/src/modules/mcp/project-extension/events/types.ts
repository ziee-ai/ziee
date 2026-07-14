// Events for the project↔mcp relationship. Lives in mcp module (the
// owner of project MCP settings post-inversion).

import type { BaseEvent } from '@ziee/framework/events'

/** Fired after a project's MCP settings are upserted via PUT
 *  /api/projects/{id}/mcp-settings. Subscribers (cache invalidators,
 *  audit listeners, etc.) react to this rather than the generic
 *  `project.updated` event so they can filter on MCP-specific changes
 *  without parsing the project payload. */
export interface ProjectMcpUpdatedEvent extends BaseEvent {
  type: 'project.mcp_updated'
  data: {
    projectId: string
  }
}

declare module '@ziee/framework/events' {
  interface AppEvents {
    'project.mcp_updated': ProjectMcpUpdatedEvent
  }
}
