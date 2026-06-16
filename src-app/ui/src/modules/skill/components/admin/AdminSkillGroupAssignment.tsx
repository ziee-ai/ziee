import { EditOutlined } from '@ant-design/icons'
import {
  App,
  Button,
  Collapse,
  Empty,
  Flex,
  Select,
  Space,
  Spin,
  Tag,
  Typography,
} from 'antd'
import { useEffect, useState } from 'react'
import { ApiClient } from '@/api-client'
import type { Group } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'

const { Text } = Typography

interface AdminSkillGroupAssignmentProps {
  skillId: string
}

/**
 * Group-assignment card for a system skill. Empty assignment = the
 * skill is available to ALL users; adding groups restricts it. Mirrors
 * McpServerGroupsAssignmentCard but inlines the editor (multi-select)
 * since skill group membership is a flat id list.
 */
export function AdminSkillGroupAssignment({
  skillId,
}: AdminSkillGroupAssignmentProps) {
  const { message } = App.useApp()
  const entry = Stores.SystemSkill.groups[skillId]
  const assignedIds = entry?.groupIds ?? []
  const loading = entry?.loading ?? false
  const canAssign = usePermission(Permissions.SkillsAssignToGroups)

  const [editing, setEditing] = useState(false)
  const [allGroups, setAllGroups] = useState<Group[]>([])
  const [draft, setDraft] = useState<string[]>([])
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    // Effect context → use `.__state` (the `Stores.X.*` proxy is
    // render-only; it calls hooks on access).
    void Stores.SystemSkill.__state.loadGroups(skillId)
  }, [skillId])

  const startEdit = async () => {
    setDraft(assignedIds)
    try {
      const res = await ApiClient.UserGroup.list({ page: 1, per_page: 100 })
      setAllGroups(res.groups)
      setEditing(true)
    } catch {
      message.error('Failed to load groups')
    }
  }

  const save = async () => {
    setSaving(true)
    try {
      await Stores.SystemSkill.setGroups(skillId, draft)
      message.success('Group assignments updated')
      setEditing(false)
    } catch {
      message.error('Failed to save group assignments')
    } finally {
      setSaving(false)
    }
  }

  const nameFor = (id: string) => allGroups.find(g => g.id === id)?.name ?? id

  return (
    <div className="pb-3" data-skill-id={skillId}>
      <Collapse
        ghost
        size="small"
        items={[
          {
            key: 'groups',
            label: <Text className="font-medium text-sm">User Groups</Text>,
            extra: canAssign ? (
              <Button
                type="text"
                size="small"
                icon={<EditOutlined aria-hidden="true" />}
                onClick={e => {
                  e.stopPropagation()
                  void startEdit()
                }}
                aria-label="Manage user groups"
              >
                Assign
              </Button>
            ) : null,
            children: loading ? (
              <Spin size="small" />
            ) : editing ? (
              <Space vertical className="w-full">
                <Select
                  mode="multiple"
                  className="w-full"
                  placeholder="Restrict to specific groups (empty = all users)"
                  value={draft}
                  onChange={setDraft}
                  options={allGroups.map(g => ({
                    label: g.name,
                    value: g.id,
                  }))}
                />
                <Flex gap={8} justify="end">
                  <Button size="small" onClick={() => setEditing(false)}>
                    Cancel
                  </Button>
                  <Button
                    size="small"
                    type="primary"
                    loading={saving}
                    onClick={save}
                  >
                    Save
                  </Button>
                </Flex>
              </Space>
            ) : assignedIds.length === 0 ? (
              <Empty
                description="Available to all users"
                image={Empty.PRESENTED_IMAGE_SIMPLE}
                className="!my-2"
              />
            ) : (
              <Space wrap size="small">
                {assignedIds.map(id => (
                  <Tag key={id} color="blue">
                    {nameFor(id)}
                  </Tag>
                ))}
              </Space>
            ),
          },
        ]}
      />
    </div>
  )
}
