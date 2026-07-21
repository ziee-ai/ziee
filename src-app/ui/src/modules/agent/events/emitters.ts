import { Stores } from '@ziee/framework/stores'
import type { AgentAdminSettings } from '@/api-client/types'

export const emitAgentAdminSettingsUpdated = async (
  settings: AgentAdminSettings,
) => {
  await Stores.EventBus.emit({
    type: 'agent.admin_settings_updated',
    data: { settings },
  })
}
