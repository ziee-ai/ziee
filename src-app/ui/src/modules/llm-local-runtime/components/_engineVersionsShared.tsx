import { type ReactNode } from 'react'
import { theme } from 'antd'

/**
 * Subtle hover background for list rows inside the engine-version
 * cards. Themed via antd's design tokens (matches the
 * AssistantMenuItem / FileAttachMenuItem pattern the chat module
 * uses): `colorFillTertiary` for hover, with a transition timed
 * from `motionDurationMid` so the feel stays consistent with the
 * rest of the design system. The negative inset + padding lets the
 * highlight extend to the Card body's inner padding edge.
 *
 * Shared between InstalledVersionsCard and AvailableVersionsCard so
 * both lists feel identical under the cursor.
 */
export function HoverRow({ children }: { children: ReactNode }) {
  const { token } = theme.useToken()
  return (
    <div
      className="rounded -mx-2 px-2 -my-1 py-1"
      style={{ transition: `background-color ${token.motionDurationMid}` }}
      onMouseEnter={e => {
        e.currentTarget.style.backgroundColor = token.colorFillTertiary
      }}
      onMouseLeave={e => {
        e.currentTarget.style.backgroundColor = 'transparent'
      }}
    >
      {children}
    </div>
  )
}

/** Human-readable byte sizes (B / KB / MB / GB). */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`
}
