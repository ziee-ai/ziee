import { Download } from 'lucide-react'
import type { MenuProps } from '@/components/ui'
import {
  Button,
  Card,
  Dropdown,
  Flex,
  MultiSelect,
  Tag,
  Text,
  message,
  Dialog,
} from '@/components/ui'
import { useState } from 'react'
import { ApiClient } from '@/api-client'
import type { Group, IndexItem } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { SkillDetailsDrawer } from './SkillDetailsDrawer'

interface SkillHubCardProps {
  item: IndexItem
}

export function SkillHubCard({ item }: SkillHubCardProps) {
  const [showDetails, setShowDetails] = useState(false)
  const [groupsOpen, setGroupsOpen] = useState(false)
  const [allGroups, setAllGroups] = useState<Group[]>([])
  const [selectedGroups, setSelectedGroups] = useState<string[]>([])
  const [submittingGroups, setSubmittingGroups] = useState(false)

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
      await Stores.HubSkills.installForGroups(item.name, selectedGroups)
      message.success(`Installed "${title}" for selected groups`)
      setGroupsOpen(false)
    } catch {
      message.error('Install failed')
    } finally {
      setSubmittingGroups(false)
    }
  }

  const adminMenu: MenuProps['items'] = [
    { key: 'me', label: 'Install for me' },
    { key: 'everyone', label: 'Install for everyone' },
    { key: 'groups', label: 'Install for groups…' },
  ];

  const handleAdminSelect = (key: string) => {
    if (key === 'me') void handleInstallForMe()
    else if (key === 'everyone') void handleInstallForEveryone()
    else if (key === 'groups') void openGroupPicker()
  }

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
          <div onClick={e => e.stopPropagation()}>
            {canManageSystem ? (
              <Dropdown
                data-testid={`hub-skill-admin-dropdown-${item.name}`}
                items={adminMenu as any}
                onSelect={handleAdminSelect}
              >
                <Button
                  variant="default"
                  icon={<Download />}
                  loading={installing}
                  disabled={installing}
                  onClick={handleInstallForMe}
                  data-testid={`hub-skill-install-dropdown-btn-${item.name}`}
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
                data-testid={`hub-skill-install-btn-${item.name}`}
              >
                Install for me
              </Button>
            ) : null}
          </div>
        </Flex>
      </Card>

      <SkillDetailsDrawer
        item={item}
        open={showDetails}
        onClose={() => setShowDetails(false)}
      />

      <Dialog
        data-testid={`hub-skill-groups-dialog-${item.name}`}
        open={groupsOpen}
        onOpenChange={(open) => { if (!open) setGroupsOpen(false) }}
        title="Install for groups"
        footer={
          <>
            <Button variant="outline" onClick={() => setGroupsOpen(false)} data-testid={`hub-skill-groups-cancel-btn-${item.name}`}>Cancel</Button>
            <Button
              variant="default"
              loading={submittingGroups}
              onClick={handleInstallForGroups}
              data-testid={`hub-skill-groups-install-btn-${item.name}`}
            >
              Install
            </Button>
          </>
        }
      >
        <MultiSelect
          data-testid={`hub-skill-groups-multiselect-${item.name}`}
          className="w-full"
          aria-label="Restrict to groups"
          placeholder="Select groups (empty = all users)"
          searchPlaceholder="Search groups…"
          value={selectedGroups}
          onChange={(value: string[]) => setSelectedGroups(value)}
          options={allGroups.map(g => ({ label: g.name, value: g.id }))}
          removeLabel={(label) => `Remove ${label}`}
          emptyText="No groups found"
        />
      </Dialog>
    </>
  )
}
