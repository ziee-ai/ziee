import type { McpComposerGet } from '../state'

/**
 * List server tools from backend.
 */
export default (_set: unknown, _get: McpComposerGet) => async (serverId: string) => {
  const { ApiClient } = await import('@/api-client')
  return await ApiClient.McpServerRuntime.listTools({ id: serverId })
}
