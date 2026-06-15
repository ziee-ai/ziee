import { Tag } from 'antd'

interface WorkflowScopeBadgeProps {
  scope: string
  isDev?: boolean
}

/** Scope chip for a workflow row: "System" / "Mine" / "Dev". */
export function WorkflowScopeBadge({ scope, isDev }: WorkflowScopeBadgeProps) {
  return (
    <>
      {scope === 'system' ? (
        <Tag color="purple">System</Tag>
      ) : (
        <Tag color="blue">Mine</Tag>
      )}
      {isDev && <Tag color="orange">Dev</Tag>}
    </>
  )
}
