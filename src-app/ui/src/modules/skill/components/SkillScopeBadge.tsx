import { Tag } from 'antd'

interface SkillScopeBadgeProps {
  scope: string
  isDev?: boolean
}

/**
 * Scope chip for a skill row: "System" / "Mine" (user scope) / "Dev"
 * (locally imported, mocks honored). A dev item also carries its base
 * scope so both chips can render.
 */
export function SkillScopeBadge({ scope, isDev }: SkillScopeBadgeProps) {
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
