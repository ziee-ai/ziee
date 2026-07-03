import { Download } from 'lucide-react'
import { Button, Card, Flex, Tag, Text, message } from '@/components/ui'
import { useState } from 'react'
import type { IndexItem } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { WorkflowDetailsDrawer } from './WorkflowDetailsDrawer'

interface WorkflowHubCardProps {
  item: IndexItem
}

export function WorkflowHubCard({ item }: WorkflowHubCardProps) {
  const [showDetails, setShowDetails] = useState(false)

  const canInstall = usePermission(Permissions.WorkflowsInstall)
  const canManageSystem = usePermission(Permissions.WorkflowsManageSystem)

  const installing = Stores.HubWorkflows.installing[item.name] ?? false
  // Derive install state from the REACTIVE installed-items field (not
  // the store's `installStateFor` function — reading a stable fn ref in
  // render subscribes to the fn key, not the underlying data, so the
  // badge would go stale on external uninstall / cross-device sync).
  const installedRows = Stores.HubInstalled.items
  const state: 'none' | 'user' | 'system' = (() => {
    const rows = installedRows.filter(
      r => r.hub_id === item.name && r.hub_category === 'workflow',
    )
    if (rows.some(r => r.is_system)) return 'system'
    if (rows.length > 0) return 'user'
    return 'none'
  })()
  const title = item.title ?? item.name

  const handleInstallForMe = async () => {
    try {
      await Stores.HubWorkflows.installForMe(item.name)
      message.success(`Installed "${title}"`)
    } catch {
      message.error('Install failed')
    }
  }

  const handleInstallForEveryone = async () => {
    try {
      await Stores.HubWorkflows.installForEveryone(item.name)
      message.success(`Installed "${title}" for everyone`)
    } catch {
      message.error('Install failed')
    }
  }

  // Same install actions as the card, rendered in the detail drawer's footer.
  const drawerFooter =
    canInstall || canManageSystem ? (
      <Flex justify="end" gap="small">
        {canInstall && (
          <Button
            variant="default"
            icon={<Download />}
            loading={installing}
            disabled={installing || state !== 'none'}
            onClick={handleInstallForMe}
            data-testid={`hub-workflow-drawer-install-btn-${item.name}`}
          >
            Install for me
          </Button>
        )}
        {canManageSystem && (
          <Button
            icon={<Download />}
            loading={installing}
            disabled={installing || state === 'system'}
            onClick={handleInstallForEveryone}
            data-testid={`hub-workflow-drawer-install-as-system-btn-${item.name}`}
          >
            {state === 'system' ? 'System installed' : 'Install as system'}
          </Button>
        )}
      </Flex>
    ) : undefined

  return (
    <>
      <Card
        hoverable
        className="cursor-pointer h-full"
        onClick={() => setShowDetails(true)}
        data-testid={`hub-workflow-card-${item.name}`}
      >
        <Flex justify="between" align="start" className="gap-3">
          <div className="flex-1 min-w-0">
            <Flex gap="small" align="center" wrap>
              <Text className="font-medium">{title}</Text>
              {item.version && (
                <Tag className="text-xs !m-0" data-testid={`hub-workflow-version-tag-${item.name}`}>v{item.version}</Tag>
              )}
              {state === 'user' && <Tag tone="success" data-testid={`hub-workflow-installed-tag-${item.name}`}>Installed</Tag>}
              {state === 'system' && <Tag tone="info" data-testid={`hub-workflow-system-tag-${item.name}`}>System installed</Tag>}
            </Flex>
            {item.summary && (
              <Text type="secondary" className="text-sm mt-1 block">
                {item.summary}
              </Text>
            )}
          </div>
          <div
            onClick={e => e.stopPropagation()}
            className="flex flex-wrap gap-1 items-center justify-end"
          >
            {canInstall && (
              <Button
                variant="default"
                icon={<Download />}
                loading={installing}
                disabled={installing || state !== 'none'}
                onClick={handleInstallForMe}
                data-testid={`hub-workflow-install-btn-${item.name}`}
              >
                Install for me
              </Button>
            )}
            {canManageSystem && (
              <Button
                icon={<Download />}
                loading={installing}
                disabled={installing || state === 'system'}
                onClick={handleInstallForEveryone}
                data-testid={`hub-workflow-install-as-system-btn-${item.name}`}
              >
                {state === 'system' ? 'System installed' : 'Install as system'}
              </Button>
            )}
          </div>
        </Flex>
      </Card>

      <WorkflowDetailsDrawer
        item={item}
        open={showDetails}
        onClose={() => setShowDetails(false)}
        footer={drawerFooter}
      />
    </>
  )
}
