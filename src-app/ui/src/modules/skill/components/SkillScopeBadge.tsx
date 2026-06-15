import { Tag } from 'antd'

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
  // antd's preset tag text colors (e.g. green #389e0d on #f6ffed = 3.37:1)
  // fail WCAG AA at the card's small font size. Darken the text to a
  // near-black shade of the same hue so contrast clears 4.5:1 while keeping
  // the preset background's color language.
  return (
    <>
      {scope === 'built_in' ? (
        <Tag color="green" style={{ color: '#0a2e00' }}>
          Built-in
        </Tag>
      ) : scope === 'system' ? (
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
