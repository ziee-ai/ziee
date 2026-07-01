import { Tag } from '@/components/ui'

interface WorkflowScopeBadgeProps {
  scope: string
  isDev?: boolean
}

/** Scope chip for a workflow row: "System" / "Mine" / "Dev". */
export function WorkflowScopeBadge({ scope, isDev }: WorkflowScopeBadgeProps) {
  // Tone colors come from the kit Tag (theme-aware, WCAG-tuned per tone).
  return (
    <>
      {scope === 'system' ? (
        <Tag variant="outline" data-testid="wf-scope-badge-system" tone="info">
          System
        </Tag>
      ) : (
        <Tag variant="outline" data-testid="wf-scope-badge-mine" tone="info">
          Mine
        </Tag>
      )}
      {isDev && (
        <Tag variant="outline" data-testid="wf-scope-badge-dev" tone="warning">
          Dev
        </Tag>
      )}
    </>
  )
}
