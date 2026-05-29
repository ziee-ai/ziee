import { Collapse, Tooltip, Typography } from 'antd'
import type { ReactNode } from 'react'

const { Text } = Typography

/**
 * Render the "Incompatible (N)" collapsed footer used by each hub tab
 * to surface items whose `min_ziee_version` exceeds the running server.
 * Pass `items` as an array of `{ id, required, content }` where
 * `content` is the unmodified card (we don't change card internals —
 * just wrap each in a Tooltip + greyed wrapper). The install button
 * inside still renders; if the user clicks it, the backend's compat
 * gate (added in a follow-up) returns the same "ziee-chat too old"
 * error. The visual styling is just an early signal.
 */
export function IncompatibleCollapse({
  items,
}: {
  items: { id: string; required: string; content: ReactNode }[]
}) {
  if (items.length === 0) return null
  return (
    <Collapse
      ghost
      items={[
        {
          key: 'incompatible',
          label: (
            <Text type="secondary">
              Incompatible ({items.length}) — require a newer ziee-chat server
            </Text>
          ),
          children: (
            <div className="flex flex-col gap-3">
              {items.map(({ id, required, content }) => (
                <Tooltip
                  key={id}
                  title={`Requires ziee-chat ≥ ${required}`}
                  placement="top"
                >
                  <div style={{ opacity: 0.55 }} aria-disabled>
                    {content}
                  </div>
                </Tooltip>
              ))}
            </div>
          ),
        },
      ]}
    />
  )
}
