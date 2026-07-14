import { useEffect, useState } from 'react'
import { Tag } from '@ziee/kit'
import type { Workflow } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { GroupEntityAssignmentDrawer } from '@/components/common/group-entity-assignment/GroupEntityAssignmentDrawer'

const workflowLabel = (w: Workflow) => w.display_name ?? w.name

/**
 * "Assign System Workflows" editor drawer on the User Groups page. Binds the
 * shared GroupEntityAssignmentDrawer to the full system-workflow list + the
 * group-centric assign endpoints.
 */
export function GroupSystemWorkflowsAssignmentDrawer() {
  const { isOpen, selectedGroup } = Stores.GroupSystemWorkflowsAssignment
  const canManage = usePermission(Permissions.WorkflowsAssignToGroups)
  const [allWorkflows, setAllWorkflows] = useState<Workflow[]>([])

  useEffect(() => {
    if (!isOpen) return
    let cancelled = false
    ApiClient.Workflow.listSystem()
      .then(res => {
        if (!cancelled)
          setAllWorkflows(Array.isArray(res.workflows) ? res.workflows : [])
      })
      .catch(err => console.error('Failed to load system workflows:', err))
    return () => {
      cancelled = true
    }
  }, [isOpen])

  return (
    <GroupEntityAssignmentDrawer<Workflow>
      isOpen={isOpen}
      group={selectedGroup}
      title="Assign System Workflows"
      testidPrefix="workflow-group-assign"
      canManage={canManage}
      allEntities={allWorkflows}
      loadAssigned={gid =>
        ApiClient.Group.getSystemWorkflows({ group_id: gid }).then(r =>
          (Array.isArray(r.workflows) ? r.workflows : []).map(w => w.id),
        )
      }
      save={(gid, ids) =>
        Stores.GroupSystemWorkflowsWidget.updateGroupWorkflows(gid, ids)
      }
      onClose={() => Stores.GroupSystemWorkflowsAssignment.closeDrawer()}
      entityLabel={workflowLabel}
      emptyText="No system workflows available"
      entityBadges={w =>
        w.enabled ? (
          <Tag
            tone="success"
            variant="outline"
            className="text-xs m-0"
            data-testid={`workflow-group-assign-status-tag-${w.id}`}
          >
            Enabled
          </Tag>
        ) : (
          <Tag
            tone="warning"
            variant="outline"
            className="text-xs m-0"
            data-testid={`workflow-group-assign-status-tag-${w.id}`}
          >
            Disabled
          </Tag>
        )
      }
    />
  )
}
