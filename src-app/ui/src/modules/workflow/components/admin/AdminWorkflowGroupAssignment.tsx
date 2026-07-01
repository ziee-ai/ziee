import { Pencil } from 'lucide-react'
import {
  Button,
  Flex,
  MultiSelect,
  Space,
  Spin,
  Tag,
  Text,
} from '@/components/ui'
import { useEffect, useState } from 'react'
import { ApiClient } from '@/api-client'
import type { Group } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'

interface AdminWorkflowGroupAssignmentProps {
  workflowId: string
}

/**
 * Group-assignment card for a system workflow. Empty assignment = the
 * workflow is available to ALL users; adding groups restricts it.
 * Mirrors AdminSkillGroupAssignment — load current groups via
 * `WorkflowSystem.getGroups`, edit + save via `WorkflowSystem.setGroups`.
 */
export function AdminWorkflowGroupAssignment({
  workflowId,
}: AdminWorkflowGroupAssignmentProps) {
  const entry = Stores.SystemWorkflow.groups[workflowId]
  const assignedIds = entry?.groupIds ?? []
  const loading = entry?.loading ?? false
  const canAssign = usePermission(Permissions.WorkflowsAssignToGroups)

  const [editing, setEditing] = useState(false)
  const [allGroups, setAllGroups] = useState<Group[]>([])
  const [draft, setDraft] = useState<string[]>([])
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    void Stores.SystemWorkflow.__state.loadGroups(workflowId)
  }, [workflowId])

  const startEdit = async () => {
    setDraft(assignedIds)
    try {
      const res = await ApiClient.UserGroup.list({ page: 1, per_page: 100 })
      setAllGroups(res.groups)
      setEditing(true)
    } catch {
      // Error handled silently
    }
  }

  const save = async () => {
    setSaving(true)
    try {
      await Stores.SystemWorkflow.setGroups(workflowId, draft)
      setEditing(false)
    } catch {
      // Error handled silently
    } finally {
      setSaving(false)
    }
  }

  const nameFor = (id: string) => allGroups.find(g => g.id === id)?.name ?? id

  return (
    <div className="pb-3" data-workflow-id={workflowId}>
      {/* Always-visible User Groups section (matches McpServerGroupsAssignmentCard):
          the "User Groups" heading with the Assign action next to it, then the
          content below. */}
      <Flex align="center" className="gap-2 mb-1">
        <Text className="font-medium text-sm">User Groups</Text>
        {canAssign && !editing && (
          <Button
            data-testid="wf-group-assign-edit-btn"
            variant="ghost"
            size="default"
            icon={<Pencil aria-hidden="true" />}
            onClick={() => void startEdit()}
            aria-label="Manage user groups"
          >
            Assign
          </Button>
        )}
      </Flex>
      {loading ? (
        <Spin size="sm" label="Loading groups" />
      ) : editing ? (
        <Space direction="vertical" className="w-full">
          <MultiSelect
            data-testid="wf-group-assign-multiselect"
            className="w-full"
            aria-label="Restrict to groups"
            placeholder="Restrict to specific groups (empty = all users)"
            searchPlaceholder="Search groups…"
            value={draft}
            onChange={setDraft}
            options={allGroups.map(g => ({
              label: g.name,
              value: g.id,
            }))}
            removeLabel={(label) => `Remove ${label}`}
            emptyText="No groups found"
          />
          <Flex gap="sm" justify="end">
            <Button data-testid="wf-group-assign-cancel-btn" size="default" variant="outline" onClick={() => setEditing(false)}>
              Cancel
            </Button>
            <Button
              data-testid="wf-group-assign-save-btn"
              size="default"
              loading={saving}
              onClick={save}
            >
              Save
            </Button>
          </Flex>
        </Space>
      ) : assignedIds.length === 0 ? (
        <Text type="secondary" className="text-xs" data-testid="wf-group-assign-empty">
          Available to all users
        </Text>
      ) : (
        <Space wrap size="middle">
          {assignedIds.map(id => (
            <Tag variant="outline" key={id} data-testid={`wf-group-assign-tag-${id}`} tone="info">
              {nameFor(id)}
            </Tag>
          ))}
        </Space>
      )}
    </div>
  )
}
