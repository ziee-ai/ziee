import type { McpServer } from '@/api-client/types'
import type { McpServerDrawerSet, McpServerDrawerPrefill, McpServerDrawerMode } from '../state'

export default (set: McpServerDrawerSet) => {
  return (
    server?: McpServer,
    mode: McpServerDrawerMode = 'create',
    prefillData?: McpServerDrawerPrefill,
  ) =>
    set({
      open: true,
      editingServer: server || null,
      prefillData: prefillData ?? null,
      isCloning: mode === 'clone',
      mode,
    })
}
