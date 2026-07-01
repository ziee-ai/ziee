import { useEffect } from 'react'
import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { UserGroupAssignment } from '@/components/common/UserGroupAssignment'

interface AdminWorkflowGroupAssignmentProps {
  workflowId: string
}

/**
 * Group-assignment section for a system workflow (empty assignment = available
 * to ALL users). Thin wrapper over the shared UserGroupAssignment component.
 */
export function AdminWorkflowGroupAssignment({
  workflowId,
}: AdminWorkflowGroupAssignmentProps) {
  const entry = Stores.SystemWorkflow.groups[workflowId]
  const assignedIds = entry?.groupIds ?? []
  const loading = entry?.loading ?? false
  const canAssign = usePermission(Permissions.WorkflowsAssignToGroups)

  useEffect(() => {
    void Stores.SystemWorkflow.__state.loadGroups(workflowId)
  }, [workflowId])

  return (
    // px-3 aligns the section with the card's p-3 header (the card is a plain
    // bordered div with no content padding of its own).
    <div data-workflow-id={workflowId} className="px-3">
      <UserGroupAssignment
        data-testid="wf-group"
        assignedGroups={assignedIds.map(id => ({ id, name: id }))}
        loading={loading}
        canAssign={canAssign}
        emptyText="Available to all users"
        editor={{
          loadAllGroups: async () => {
            const res = await ApiClient.UserGroup.list({ page: 1, per_page: 100 })
            return res.groups.map(g => ({ id: g.id, name: g.name }))
          },
          save: ids => Stores.SystemWorkflow.setGroups(workflowId, ids),
        }}
      />
    </div>
  )
}
