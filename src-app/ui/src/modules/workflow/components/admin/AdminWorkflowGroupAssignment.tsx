import { Pencil } from 'lucide-react'
import {
  Button,
  Accordion,
  Empty,
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
      <Accordion
        collapsible
        items={[
          {
            key: 'groups',
            label: <Text className="font-medium text-sm">User Groups</Text>,
            children: loading ? (
              <Spin size="sm" label="Loading groups" />
            ) : editing ? (
              <Space direction="vertical" className="w-full">
                <MultiSelect
                  className="w-full"
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
                  <Button size="sm" variant="outline" onClick={() => setEditing(false)}>
                    Cancel
                  </Button>
                  <Button
                    size="sm"
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
                className="!my-2"
              />
            ) : (
              <Space wrap size="middle">
                {assignedIds.map(id => (
                  <Tag key={id} tone="info">
                    {nameFor(id)}
                  </Tag>
                ))}
              </Space>
            ),
          },
        ]}
      />
      {canAssign && !editing && assignedIds.length > 0 && (
        <div className="flex justify-end px-4 pb-2">
          <Button
            variant="ghost"
            size="sm"
            icon={<Pencil aria-hidden="true" />}
            onClick={() => void startEdit()}
            aria-label="Manage user groups"
          >
            Assign
          </Button>
        </div>
      )}
    </div>
  )
}
