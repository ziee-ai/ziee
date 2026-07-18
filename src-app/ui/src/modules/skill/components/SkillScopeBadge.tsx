import { Tag } from '@ziee/kit'

interface SkillScopeBadgeProps {
  scope: string
  isDev?: boolean
}

/**
 * Scope chip for a skill row: "Built-in" (ziee's embedded capability
 * skills — always on, not uninstallable) / "System" / "Mine" (user scope)
 * / "Dev" (locally imported, mocks honored). A dev item also carries its
 * base scope so both chips can render.
 */
export function SkillScopeBadge({ scope, isDev }: SkillScopeBadgeProps) {
  // Tone colors come from the kit Tag (theme-aware, WCAG-tuned per tone).
  return (
    <>
      {scope === 'built_in' ? (
        <Tag variant="outline" tone="success" data-testid="skill-scope-badge-builtin">
          Built-in
        </Tag>
      ) : scope === 'system' ? (
        <Tag variant="outline" tone="info" data-testid="skill-scope-badge-system">
          System
        </Tag>
      ) : (
        <Tag variant="outline" tone="info" data-testid="skill-scope-badge-mine">
          Mine
        </Tag>
      )}
      {isDev && (
        <Tag variant="outline" tone="warning" data-testid="skill-scope-badge-dev">
          Dev
        </Tag>
      )}
    </>
  )
}
