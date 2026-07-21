import { projectConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Open the config modal in PROJECT scope. Seeds a config under
 * `projectConfigKey(projectId)` and clears currentConversationId.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  projectId: string,
  settings: {
    auto_approved_tools?: unknown
    disabled_servers?: unknown
    loop_settings?: unknown
    approval_mode?: string
  } | null,
) => {
  const key = projectConfigKey(projectId)
  set(state => {
    const autoApprovedRaw = (settings?.auto_approved_tools as
      | import('@/api-client/types').AutoApprovedServer[]
      | undefined) ?? []
    const disabledRaw = (settings?.disabled_servers as
      | import('@/api-client/types').DisabledServer[]
      | undefined) ?? []
    const loop = (settings?.loop_settings as
      | import('@/api-client/types').LoopSettings
      | null
      | undefined) ?? undefined

    // selectedServers stays empty here; the modal computes per-server
    // selection from disabledRaw + the live enabled-server list it
    // already loads (`selection = !disabledServers.find(...)`).
    const selectedServers = new Map<string, { server_id: string; tools: string[] }>()

    state.conversationConfigs.set(key, {
      selectedServers,
      disabledServers: disabledRaw,
      approvalMode: (settings?.approval_mode as
        | 'disabled'
        | 'auto_approve'
        | 'manual_approve') || 'manual_approve',
      autoApprovedTools: autoApprovedRaw,
      loopSettings: loop,
    })

    // RESET the GLOBAL `selectedServers` Map.
    state.selectedServers = new Map()

    state.currentProjectId = projectId
    state.currentConversationId = null
    state.configModalVisible = true
  })
}
