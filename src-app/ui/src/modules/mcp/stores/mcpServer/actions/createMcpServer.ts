import { ApiClient } from '@/api-client'
import { type CreateMcpServerRequest, type McpServerWithHealthWarning } from '@/api-client/types'
import { emitMcpServerCreated } from '@/modules/mcp/events'
import type { McpServerGet, McpServerSet } from '../state'

export default (set: McpServerSet, _get: McpServerGet) =>
  async (data: CreateMcpServerRequest): Promise<McpServerWithHealthWarning> => {
    try {
      set(draft => {
        draft.creating = true
        draft.error = null
      })
      // Flattened response: McpServer fields at top level + optional
      // `connection_warning` sibling (post-create probe auto-downgrade).
      const wrapped = await ApiClient.McpServer.create(data)
      const { connection_warning: _w, ...newServer } = wrapped
      try {
        await emitMcpServerCreated(newServer)
      } catch (eventError) {
        console.error('Failed to emit mcp server created event:', eventError)
      }
      set({ creating: false })
      return wrapped
    } catch (error) {
      console.error('MCP server creation failed:', error)
      set(draft => {
        draft.creating = false
        draft.error = error instanceof Error ? error.message : 'Failed to create MCP server'
      })
      throw error
    }
  }
