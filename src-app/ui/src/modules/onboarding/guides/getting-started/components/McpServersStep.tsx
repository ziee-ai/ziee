import { useEffect } from 'react'
import {
  Title,
  Paragraph,
  Text,
  Checkbox,
  Spin,
  Alert,
  Separator,
  Tag,
  Switch,
} from '@ziee/kit'
import { Wrench } from 'lucide-react'
import type { OnboardingStepProps } from '@/modules/onboarding/types/onboarding'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'

export default function McpServersStep({ registerBeforeNext }: OnboardingStepProps) {
  const selectedMcpServerIds = Stores.McpServersStep.selectedMcpServerIds
  const { systemServers, hubServers, installedNames, loadingServers, serversError, disabledSystemIds } = Stores.McpServersStep

  // The step renders for every authenticated user, but the controls are
  // admin-only. Non-admins see just the intro paragraph and continue.
  const canManageSystemMcp = usePermission(Permissions.McpServersAdminEdit)
  const canInstallFromHub = usePermission(Permissions.HubMcpServersCreate)
  const canSeeAdminControls = canManageSystemMcp || canInstallFromHub

  useEffect(() => {
    Stores.Onboarding.setReady(true)
    if (canSeeAdminControls) {
      // Wire up the before-next handler so hub-server installations AND
      // system-server toggles are persisted when the user clicks Next/Start Chatting.
      registerBeforeNext(() => Stores.McpServersStep.applyMcpServerChanges())
      Stores.McpServersStep.loadMcpServers()
    } else {
      registerBeforeNext(null)
    }
  }, [canSeeAdminControls])

  if (loadingServers) {
    return (
      <div className="flex justify-center mt-8">
        <Spin label="Loading" />
      </div>
    )
  }

  return (
    <div className="max-w-xl">
      <div className="flex items-center gap-3 mb-4">
        <Wrench className="text-3xl text-primary" />
        <Title level={3} className="!mb-0">
          MCP Servers
        </Title>
      </div>

      <Paragraph tone="secondary">
        {canSeeAdminControls
          ? 'MCP servers extend your AI assistant with tools and data access. Toggle the ones you want to use, or install new ones from the Hub.'
          : 'MCP servers extend your AI assistant with tools and data access. Your administrator has already configured the servers available to you.'}
      </Paragraph>

      {serversError && canSeeAdminControls && (
        <Alert data-testid="onboarding-mcp-error-alert" tone="error" title={serversError} className="mb-4" />
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
                data-testid={`onboarding-mcp-system-server-row-${server.id}`}
                className="flex items-start gap-3 border rounded-lg p-3"
              >
                <Switch
                  tooltip="Enable this server"
                  data-testid={`onboarding-mcp-system-server-switch-${server.id}`}
                  size="sm"
                  checked={!disabledSystemIds.has(server.id)}
                  onChange={checked => Stores.McpServersStep.toggleSystemServer(server.id, checked)}
                  className="mt-1"
                />
                <div>
                  <Text strong>{server.display_name || server.name}</Text>
                  {server.description && (
                    <Text tone="secondary" className="block text-sm">
                      {server.description}
                    </Text>
                  )}
                </div>
              </div>
            ))}
          </div>
          <Separator />
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
              const isSelected = selectedMcpServerIds.includes(server.name)
              // v2: derive display label from the reverse-DNS leaf (the
              // strict server.json shape no longer carries display_name).
              const leaf = (() => {
                const slash = server.name.indexOf('/')
                return slash >= 0 ? server.name.slice(slash + 1) : server.name
              })()
              return (
                <div
                  key={server.name}
                  className={`flex items-start gap-3 border rounded-lg p-3 ${
                    alreadyInstalled
                      ? 'opacity-50 cursor-not-allowed'
                      : 'cursor-pointer hover:bg-accent'
                  }`}
                  onClick={alreadyInstalled ? undefined : () => Stores.McpServersStep.toggleMcpServer(server.name)}
                >
                  {/* stop bubbling so the checkbox's own toggle doesn't double-fire the row onClick */}
                  <span onClick={e => e.stopPropagation()}>
                    <Checkbox
                      data-testid={`onboarding-mcp-hub-server-checkbox-${server.name}`}
                      checked={isSelected}
                      disabled={alreadyInstalled}
                      onChange={() => Stores.McpServersStep.toggleMcpServer(server.name)}
                      className="mt-1"
                    />
                  </span>
                  <div>
                    <div className="flex items-center gap-2">
                      <Text strong>{leaf}</Text>
                      {alreadyInstalled && <Tag variant="outline" data-testid={`onboarding-mcp-hub-server-installed-tag-${server.name}`}>Already installed</Tag>}
                    </div>
                    {server.description && (
                      <Text tone="secondary" className="block text-sm">
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
