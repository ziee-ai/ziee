import { DownloadOutlined } from '@ant-design/icons'
import type { MenuProps } from 'antd'
import {
  App,
  Button,
  Card,
  Dropdown,
  Flex,
  Modal,
  Select,
  Tag,
  Typography,
} from 'antd'
import { useState } from 'react'
import { ApiClient } from '@/api-client'
import type { Group, IndexItem } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { SkillDetailsDrawer } from './SkillDetailsDrawer'

const { Text } = Typography

interface SkillHubCardProps {
  item: IndexItem
}

export function SkillHubCard({ item }: SkillHubCardProps) {
  const { message } = App.useApp()
  const [showDetails, setShowDetails] = useState(false)
  const [groupsOpen, setGroupsOpen] = useState(false)
  const [allGroups, setAllGroups] = useState<Group[]>([])
  const [selectedGroups, setSelectedGroups] = useState<string[]>([])
  const [submittingGroups, setSubmittingGroups] = useState(false)

  const canInstall = usePermission(Permissions.SkillsInstall)
  const canManageSystem = usePermission(Permissions.SkillsManageSystem)

  const installing = Stores.HubSkills.installing[item.name] ?? false
  // Derive install state from the REACTIVE installed-items field (not
  // the store's `installStateFor` function — reading a stable fn ref in
  // render subscribes to the fn key, not the underlying data, so the
  // badge would go stale on external uninstall / cross-device sync).
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

  const adminMenu: MenuProps = {
    items: [
      { key: 'me', label: 'Install for me' },
      { key: 'everyone', label: 'Install for everyone' },
      { key: 'groups', label: 'Install for groups…' },
    ],
    onClick: ({ key }) => {
      if (key === 'me') void handleInstallForMe()
      else if (key === 'everyone') void handleInstallForEveryone()
      else if (key === 'groups') void openGroupPicker()
    },
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
        <Flex justify="space-between" align="flex-start" gap={12}>
          <div className="flex-1 min-w-0">
            <Flex gap={8} align="center" wrap>
              <Text className="font-medium">{title}</Text>
              {item.version && (
                <Tag className="text-xs !m-0">v{item.version}</Tag>
              )}
              {state === 'user' && <Tag color="green">Installed</Tag>}
              {state === 'system' && <Tag color="purple">System installed</Tag>}
            </Flex>
            {item.summary && (
              <Text type="secondary" className="text-sm mt-1 block">
                {item.summary}
              </Text>
            )}
          </div>
          <div onClick={e => e.stopPropagation()}>
            {canManageSystem ? (
              <Dropdown.Button
                type="primary"
                icon={<DownloadOutlined />}
                loading={installing}
                disabled={installing}
                menu={adminMenu}
                onClick={handleInstallForMe}
              >
                Install
              </Dropdown.Button>
            ) : canInstall ? (
              <Button
                type="primary"
                icon={<DownloadOutlined />}
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

      <SkillDetailsDrawer
        item={item}
        open={showDetails}
        onClose={() => setShowDetails(false)}
      />

      <Modal
        open={groupsOpen}
        title="Install for groups"
        onCancel={() => setGroupsOpen(false)}
        onOk={handleInstallForGroups}
        okText="Install"
        confirmLoading={submittingGroups}
      >
        <Select
          mode="multiple"
          className="w-full"
          placeholder="Select groups (empty = all users)"
          value={selectedGroups}
          onChange={setSelectedGroups}
          options={allGroups.map(g => ({ label: g.name, value: g.id }))}
        />
      </Modal>
    </>
  )
}
