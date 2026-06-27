import { Tag } from '@/components/ui'

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
        <Tag tone="info" className="text-[#2c0a6b]">
          System
        </Tag>
      ) : (
        <Tag tone="info" className="text-[#001a4d]">
          Mine
        </Tag>
      )}
      {isDev && (
        <Tag tone="warning" className="text-[#612500]">
          Dev
        </Tag>
      )}
    </>
  )
}
