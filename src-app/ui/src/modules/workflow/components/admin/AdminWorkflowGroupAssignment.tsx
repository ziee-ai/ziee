import { Collapse, Typography } from 'antd'

const { Text } = Typography

interface AdminWorkflowGroupAssignmentProps {
  workflowId: string
}

/**
 * Group-restriction info for a system workflow. The backend assigns
 * groups at install time (the hub card's "Install for groups…" option);
 * post-install group editing isn't exposed by the generated API, so
 * this card documents the current behavior. Re-install with a different
 * group set to change the restriction.
 */
export function AdminWorkflowGroupAssignment({
  workflowId,
}: AdminWorkflowGroupAssignmentProps) {
  return (
    <div className="pb-3" data-workflow-id={workflowId}>
      <Collapse
        ghost
        size="small"
        items={[
          {
            key: 'groups',
            label: <Text className="font-medium text-sm">User Groups</Text>,
            children: (
              <Text type="secondary" className="text-xs">
                Group restrictions are set when installing from the Hub
                ("Install for groups…"). Empty = available to all users.
                Re-install to change the assignment.
              </Text>
            ),
          },
        ]}
      />
    </div>
  )
}
