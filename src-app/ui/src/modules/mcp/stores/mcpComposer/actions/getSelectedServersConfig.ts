import type { McpComposerGet } from '../state'
import type { McpServerConfig } from '@/api-client/types'

/** Get selected servers config for request — synchronous. */
export default (_set: unknown, get: McpComposerGet): () => McpServerConfig[] => {
  return (): McpServerConfig[] => {
    const selections = Array.from(get().selectedServers.values())
    return selections.map(sel => ({
      server_id: sel.server_id,
      tools: sel.tools.length > 0 ? sel.tools : undefined,
    }))
  }
}
