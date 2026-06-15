import { Tag } from 'antd'

interface WorkflowScopeBadgeProps {
  scope: string
  isDev?: boolean
}

/** Scope chip for a workflow row: "System" / "Mine" / "Dev". */
export function WorkflowScopeBadge({ scope, isDev }: WorkflowScopeBadgeProps) {
  // Darken antd preset tag text to clear WCAG AA contrast (see
  // SkillScopeBadge — the preset text colors fail at the card's small font).
  return (
    <>
      {scope === 'system' ? (
        <Tag color="purple" style={{ color: '#2c0a6b' }}>
          System
        </Tag>
      ) : (
        <Tag color="blue" style={{ color: '#001a4d' }}>
          Mine
        </Tag>
      )}
      {isDev && (
        <Tag color="orange" style={{ color: '#612500' }}>
          Dev
        </Tag>
      )}
    </>
  )
}
