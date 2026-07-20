import { Download } from 'lucide-react'
import { Button, Card, Flex, Tag, Text, message } from '@ziee/kit'
import { useState } from 'react'
import type { IndexItem } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { usePermission } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'
import { SkillDetailsDrawer } from './SkillDetailsDrawer'

interface SkillHubCardProps {
  item: IndexItem
}

export function SkillHubCard({ item }: SkillHubCardProps) {
  const [showDetails, setShowDetails] = useState(false)

  const canInstall = usePermission(Permissions.SkillsInstall)
  const canManageSystem = usePermission(Permissions.SkillsManageSystem)

  const installing = Stores.HubSkills.installing[item.name] ?? false
  const installedRows = Stores.HubInstalled.items
  const state: 'none' | 'user' | 'system' = (() => {
    const rows = installedRows.filter(
      r => r.hub_id === item.name && r.hub_category === 'skill',
    )
    if (rows.some(r => r.is_system)) return 'system'
    if (rows.length > 0) return 'user'
    return 'none'
  })()
  const title = item.title ?? item.name

  const handleInstallForMe = async () => {
    try {
      await Stores.HubSkills.installForMe(item.name)
      message.success(`Installed "${title}"`)
    } catch {
      message.error('Install failed')
    }
  }

  const handleInstallForEveryone = async () => {
    try {
      await Stores.HubSkills.installForEveryone(item.name)
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
            data-testid={`hub-skill-drawer-install-btn-${item.name}`}
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
            data-testid={`hub-skill-drawer-install-as-system-btn-${item.name}`}
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
        className="cursor-pointer h-full focus-visible:outline focus-visible:outline-2"
        role="button"
        tabIndex={0}
        aria-label={`View skill ${item.name}`}
        onClick={() => setShowDetails(true)}
        onKeyDown={e => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            setShowDetails(true)
          }
        }}
        data-testid={`hub-skill-card-${item.name}`}
      >
        <Flex justify="between" align="baseline" className="gap-4">
          <div className="flex-1 min-w-0">
            <Flex gap="small" align="center" wrap>
              <Text className="font-medium">{title}</Text>
              {item.version && (
                <Tag className="text-xs !m-0" data-testid={`hub-skill-version-tag-${item.name}`}>v{item.version}</Tag>
              )}
              {state === 'user' && <Tag tone="success" data-testid={`hub-skill-installed-tag-${item.name}`}>Installed</Tag>}
              {state === 'system' && <Tag tone="info" data-testid={`hub-skill-system-tag-${item.name}`}>System installed</Tag>}
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
                data-testid={`hub-skill-install-btn-${item.name}`}
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
                data-testid={`hub-skill-install-as-system-btn-${item.name}`}
              >
                {state === 'system' ? 'System installed' : 'Install as system'}
              </Button>
            )}
          </div>
        </Flex>
      </Card>

      <SkillDetailsDrawer
        item={item}
        open={showDetails}
        onClose={() => setShowDetails(false)}
        footer={drawerFooter}
      />
    </>
  )
}
