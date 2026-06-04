import { Spin, Typography } from 'antd'
import type { SpinProps } from 'antd'
import type { ReactNode } from 'react'

export interface LoadingProps extends Omit<SpinProps, 'children' | 'tip'> {
  /** Fill the viewport height (min-h-screen) — for the app-bootstrap loader. */
  fullscreen?: boolean
  /** Optional label rendered under the spinner. */
  tip?: ReactNode
  /** Extra classes on the centering wrapper. */
  className?: string
}

/**
 * Standard app loading indicator: an antd <Spin> centered in its container,
 * with an optional label below it.
 *
 * Use this wherever a page / route / section / card is loading so the spinner
 * icon, size, and centering stay consistent across the whole UI. Defaults to
 * `size="large"`; pass `size="small"` for inline / widget spots, or
 * `fullscreen` for the app-bootstrap loader (AuthGuard).
 */
export function Loading({
  fullscreen = false,
  size = 'large',
  tip,
  className = '',
  ...spin
}: LoadingProps) {
  return (
    <div
      className={`flex flex-col items-center justify-center gap-3 ${
        fullscreen ? 'min-h-screen' : 'h-full w-full py-8'
      } ${className}`.trim()}
    >
      <Spin size={size} {...spin} />
      {tip != null && <Typography.Text type="secondary">{tip}</Typography.Text>}
    </div>
  )
}
