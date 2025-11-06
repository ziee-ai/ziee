import { useEffect } from 'react'
import { App } from 'antd'
import { SystemServersTab } from './SystemServersTab'
import { Stores } from '@/core/stores'
import { loadSystemServers } from '../../store'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

export function SystemMcpServersPage() {
  const { message } = App.useApp()
  const { systemServersInitialized } = Stores.SystemMcpServer

  useEffect(() => {
    // Initialize system MCP servers store when component mounts
    const initializeStores = async () => {
      try {
        if (!systemServersInitialized) {
          await loadSystemServers()
        }
      } catch (error) {
        console.error('Failed to initialize system MCP servers:', error)
        message.error('Failed to load system MCP servers')
      }
    }

    initializeStores()
  }, [systemServersInitialized, message])

  return (
    <SettingsPageContainer
      title="System MCP Servers"
      subtitle="Manage Model Context Protocol servers across the system"
    >
      <SystemServersTab />
    </SettingsPageContainer>
  )
}
