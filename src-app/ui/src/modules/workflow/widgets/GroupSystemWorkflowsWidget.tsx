import { useEffect } from 'react'
import { Workflow as WorkflowIcon } from 'lucide-react'
import type { GroupWidgetProps } from '@/modules/user/types/GroupWidget'
import type { Workflow } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { usePermission } from '@/core/permissions'
import { GroupEntityAssignmentWidget } from '@/components/common/group-entity-assignment/GroupEntityAssignmentWidget'
import { GroupSystemWorkflowsAssignment } from '@/modules/workflow/widgets/groupSystemWorkflowsAssignmentDrawer'
import { GroupSystemWorkflowsWidget as GroupSystemWorkflowsWidgetStore } from '@/modules/workflow/widgets/groupSystemWorkflowsWidget'

const workflowLabel = (w: Workflow) => w.display_name ?? w.name

/**
 * "System Workflows" assignment widget on the User Groups page. Thin binding
 * of the shared GroupEntityAssignmentWidget to the workflow widget store.
 */
export function GroupSystemWorkflowsWidget({ group }: GroupWidgetProps) {
  const data = GroupSystemWorkflowsWidgetStore.groupWorkflows.get(group.id)
  const canManage = usePermission(Permissions.WorkflowsAssignToGroups)

  // The group-system-workflows read endpoint requires workflows::assign_to_groups
  // (same as canManage). Gate the eager load so a groups::read-only admin
  // without it doesn't 403 on mount.
  useEffect(() => {
    if (canManage) GroupSystemWorkflowsWidgetStore.loadWorkflowsForGroup(group.id)
  }, [group.id, canManage])

  return (
    <GroupEntityAssignmentWidget<Workflow>
      group={group}
      title="System Workflows"
      icon={<WorkflowIcon className="text-primary" aria-hidden="true" />}
      testidPrefix="workflow-group-widget"
      canManage={canManage}
      data={
        data
          ? {
              entities: data.workflows,
              loading: data.loading,
              error: data.error,
            }
          : undefined
      }
      onEdit={g => GroupSystemWorkflowsAssignment.openDrawer(g)}
      entityLabel={workflowLabel}
      entityActive={w => w.enabled}
    />
  )
}
