import { Download } from 'lucide-react'
import {
  Button,
  Card,
  Dropdown,
  Flex,
  Dialog,
  Tag,
  Text,
  message,
  MultiSelect,
} from '@/components/ui'
import { useState } from 'react'
import { ApiClient } from '@/api-client'
import type { Group, IndexItem } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { WorkflowDetailsDrawer } from './WorkflowDetailsDrawer'

interface WorkflowHubCardProps {
  item: IndexItem
}

const adminMenuItems = [
  { key: 'me', label: 'Install for me' },
  { key: 'everyone', label: 'Install for everyone' },
  { key: 'groups', label: 'Install for groups…' },
]

export function WorkflowHubCard({ item }: WorkflowHubCardProps) {
  const [showDetails, setShowDetails] = useState(false)
  const [groupsOpen, setGroupsOpen] = useState(false)
  const [allGroups, setAllGroups] = useState<Group[]>([])
  const [selectedGroups, setSelectedGroups] = useState<string[]>([])
  const [submittingGroups, setSubmittingGroups] = useState(false)

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

  const openGroupPicker = async () => {
    try {
      const res = await ApiClient.UserGroup.list({ page: 1, per_page: 100 })
      setAllGroups(res.groups)
      setSelectedGroups([])
      setGroupsOpen(true)
    } catch {
      message.error('Failed to load groups')
    }
  }

  const handleInstallForGroups = async () => {
    setSubmittingGroups(true)
    try {
      await Stores.HubWorkflows.installForGroups(item.name, selectedGroups)
      message.success(`Installed "${title}" for selected groups`)
      setGroupsOpen(false)
    } catch {
      message.error('Install failed')
    } finally {
      setSubmittingGroups(false)
    }
  }

  const handleInstallAction = (key: string) => {
    if (key === 'me') void handleInstallForMe()
    else if (key === 'everyone') void handleInstallForEveryone()
    else if (key === 'groups') void openGroupPicker()
  }

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
                <Tag className="text-xs !m-0">v{item.version}</Tag>
              )}
              {state === 'user' && <Tag tone="success">Installed</Tag>}
              {state === 'system' && <Tag tone="info">System installed</Tag>}
            </Flex>
            {item.summary && (
              <Text type="secondary" className="text-sm mt-1 block">
                {item.summary}
              </Text>
            )}
          </div>
          <div onClick={e => e.stopPropagation()}>
            {canManageSystem ? (
              <Dropdown items={adminMenuItems} onSelect={handleInstallAction}>
                <Button
                  variant="default"
                  icon={<Download />}
                  loading={installing}
                  disabled={installing}
                  onClick={handleInstallForMe}
                >
                  Install
                </Button>
              </Dropdown>
            ) : canInstall ? (
              <Button
                variant="default"
                icon={<Download />}
                loading={installing}
                disabled={state !== 'none'}
                onClick={handleInstallForMe}
              >
                Install for me
              </Button>
            ) : null}
          </div>
        </Flex>
      </Card>

      <WorkflowDetailsDrawer
        item={item}
        open={showDetails}
        onClose={() => setShowDetails(false)}
      />

      <Dialog
        open={groupsOpen}
        title="Install for groups"
        onOpenChange={(open) => {
          if (!open) setGroupsOpen(false)
        }}
        footer={
          <>
            <Button variant="outline" onClick={() => setGroupsOpen(false)}>
              Cancel
            </Button>
            <Button
              variant="default"
              loading={submittingGroups}
              onClick={handleInstallForGroups}
            >
              Install
            </Button>
          </>
        }
      >
        <MultiSelect
          className="w-full"
          placeholder="Select groups (empty = all users)"
          searchPlaceholder="Search groups…"
          emptyText="No groups found"
          value={selectedGroups}
          onChange={setSelectedGroups}
          options={allGroups.map(g => ({ label: g.name, value: g.id }))}
          removeLabel={(label) => `Remove ${label}`}
        />
      </Dialog>
    </>
  )
}
