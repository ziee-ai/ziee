import { EventBus } from '@ziee/framework/stores'
import type { AgentAdminSettings } from '@/api-client/types'

export const emitAgentAdminSettingsUpdated = async (
  settings: AgentAdminSettings,
) => {
  await EventBus.emit({
    type: 'agent.admin_settings_updated',
    data: { settings },
  })
}
