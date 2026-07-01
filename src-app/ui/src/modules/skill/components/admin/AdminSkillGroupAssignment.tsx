import { Pencil, ChevronDown, ChevronRight } from 'lucide-react'
import { useEffect, useState } from 'react'
import { ApiClient } from '@/api-client'
import type { Group } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import {
  message,
  Button,
  Empty,
  Flex,
  MultiSelect,
  Space,
  Spin,
  Tag,
  Text,
} from '@/components/ui'

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
    const state = Stores.SystemSkill.__state
    const existing = state.groups[skillId]
    // Skip re-fetch on re-mount if groups are already loaded or loading.
    if (existing) return
    void state.loadGroups(skillId)
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

  const [open, setOpen] = useState(false)

  return (
    <div className="pb-3" data-skill-id={skillId}>
      {/* Lightweight disclosure (was an antd Collapse with a header `extra`
          action — kit Accordion has no extra slot + nests a <button> in its
          trigger, so a manual header row is used instead). */}
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="default"
          data-testid="skill-group-toggle-button"
          onClick={() => setOpen(o => !o)}
          aria-expanded={open}
          aria-label={open ? 'Collapse user groups' : 'Expand user groups'}
          icon={open ? <ChevronDown aria-hidden="true" /> : <ChevronRight aria-hidden="true" />}
        >
          <Text className="font-medium text-sm">User Groups</Text>
        </Button>
        <div className="ml-auto">
          {canAssign ? (
            <Button
              variant="ghost"
              size="default"
              data-testid="skill-group-assign-button"
              icon={<Pencil aria-hidden="true" />}
              onClick={() => { setOpen(true); void startEdit() }}
              aria-label="Manage user groups"
            >
              Assign
            </Button>
          ) : null}
        </div>
      </div>
      {open && (
        <div className="pt-2">
          {loading ? (
              <Spin size="sm" label="Loading" />
            ) : editing ? (
              <Space direction="vertical" className="w-full">
                <MultiSelect
                  className="w-full"
                  data-testid="skill-group-multiselect"
                  placeholder="Restrict to specific groups (empty = all users)"
                  searchPlaceholder="Search groups"
                  emptyText="No groups found"
                  removeLabel={label => `Remove ${label}`}
                  value={draft}
                  onChange={setDraft}
                  options={allGroups.map(g => ({
                    label: g.name,
                    value: g.id,
                  }))}
                  aria-label="Select groups"
                />
                <Flex gap="small" justify="end">
                  <Button size="default" variant="outline" data-testid="skill-group-cancel-button" onClick={() => setEditing(false)}>
                    Cancel
                  </Button>
                  <Button
                    size="default"
                    loading={saving}
                    data-testid="skill-group-save-button"
                    onClick={save}
                  >
                    Save
                  </Button>
                </Flex>
              </Space>
            ) : assignedIds.length === 0 ? (
              <Empty
                description="Available to all users"
                className="!my-2"
                data-testid="skill-group-empty"
              />
            ) : (
              <Space wrap size="small">
                {assignedIds.map(id => (
                  <Tag key={id} tone="info" data-testid={`skill-group-tag-${id}`}>
                    {nameFor(id)}
                  </Tag>
                ))}
              </Space>
            )}
        </div>
      )}
    </div>
  )
}
