import { useEffect } from 'react'
import {
  Typography,
  Checkbox,
  Spin,
  Alert,
  Divider,
  Tag,
  Switch,
} from 'antd'
import { ToolOutlined } from '@ant-design/icons'
import type { OnboardingStepProps } from '@/modules/onboarding/types/onboarding'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const { Title, Paragraph, Text } = Typography

export default function McpServersStep({ registerBeforeNext }: OnboardingStepProps) {
  const selectedMcpServerIds = Stores.McpServersStep.selectedMcpServerIds
  const { systemServers, hubServers, installedNames, loadingServers, serversError } = Stores.McpServersStep

  // The step renders for every authenticated user, but the controls are
  // admin-only. Non-admins see just the intro paragraph and continue.
  const canManageSystemMcp = usePermission(Permissions.McpServersAdminEdit)
  const canInstallFromHub = usePermission(Permissions.HubMcpServersCreate)
  const canSeeAdminControls = canManageSystemMcp || canInstallFromHub

  useEffect(() => {
    Stores.Onboarding.setReady(true)
    registerBeforeNext(null)
    if (canSeeAdminControls) {
      Stores.McpServersStep.loadMcpServers()
    }
  }, [canSeeAdminControls])

  if (loadingServers) {
    return (
      <div className="flex justify-center mt-8">
        <Spin />
      </div>
    )
  }

  return (
    <div className="max-w-xl">
      <div className="flex items-center gap-3 mb-4">
        <ToolOutlined className="text-3xl text-purple-500" />
        <Title level={3} className="!mb-0">
          MCP Servers
        </Title>
      </div>

      <Paragraph type="secondary">
        {canSeeAdminControls
          ? 'MCP servers extend your AI assistant with tools and data access. Toggle the ones you want to use, or install new ones from the Hub.'
          : 'MCP servers extend your AI assistant with tools and data access. Your administrator has already configured the servers available to you.'}
      </Paragraph>

      {serversError && canSeeAdminControls && (
        <Alert type="error" title={serversError} showIcon className="mb-4" />
      )}

      {canManageSystemMcp && systemServers.length > 0 && (
        <>
          <Text strong className="block mb-2">
            System Servers
          </Text>
          <div className="space-y-2 mb-4">
            {systemServers.map(server => (
              <div
                key={server.id}
                className="flex items-start gap-3 border rounded-lg p-3"
              >
                <Switch
                  size="small"
                  defaultChecked
                  onChange={checked => Stores.McpServersStep.toggleSystemServer(server.id, checked)}
                  className="mt-1"
                />
                <div>
                  <Text strong>{server.display_name || server.name}</Text>
                  {server.description && (
                    <Text type="secondary" className="block text-sm">
                      {server.description}
                    </Text>
                  )}
                </div>
              </div>
            ))}
          </div>
          <Divider />
        </>
      )}

      {canInstallFromHub && hubServers.length > 0 && (
        <>
          <Text strong className="block mb-2">
            Install from Hub
          </Text>
          <div className="space-y-2 mb-4">
            {hubServers.slice(0, 10).map(server => {
              const alreadyInstalled = installedNames.has(server.name)
              const isSelected = selectedMcpServerIds.includes(server.id)
              return (
                <div
                  key={server.id}
                  className={`flex items-start gap-3 border rounded-lg p-3 ${
                    alreadyInstalled
                      ? 'opacity-50 cursor-not-allowed'
                      : 'cursor-pointer hover:bg-gray-50'
                  }`}
                  onClick={alreadyInstalled ? undefined : () => Stores.McpServersStep.toggleMcpServer(server.id)}
                >
                  <Checkbox
                    checked={isSelected}
                    disabled={alreadyInstalled}
                    className="mt-1"
                  />
                  <div>
                    <div className="flex items-center gap-2">
                      <Text strong>{server.display_name || server.name}</Text>
                      {alreadyInstalled ? (
                        <Tag>Already installed</Tag>
                      ) : (
                        server.category && (
                          <Tag color="blue">{server.category}</Tag>
                        )
                      )}
                    </div>
                    {server.description && (
                      <Text type="secondary" className="block text-sm">
                        {server.description}
                      </Text>
                    )}
                  </div>
                </div>
              )
            })}
          </div>
        </>
      )}
    </div>
  )
}
