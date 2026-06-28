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
        <Tag data-testid="wf-scope-badge-system" tone="info" className="text-[#2c0a6b]">
          System
        </Tag>
      ) : (
        <Tag data-testid="wf-scope-badge-mine" tone="info" className="text-[#001a4d]">
          Mine
        </Tag>
      )}
      {isDev && (
        <Tag data-testid="wf-scope-badge-dev" tone="warning" className="text-[#612500]">
          Dev
        </Tag>
      )}
    </>
  )
}
