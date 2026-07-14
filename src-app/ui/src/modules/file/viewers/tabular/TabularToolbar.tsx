import { Fragment, useState } from 'react'
import { Button, InputNumber, Text } from '@ziee/kit'

/**
 * The tabular viewer's "N rows" readout + jump-to-row control. Rendered INTO the
 * kit Table's own toolbar via its `toolbarExtra` slot (trailing edge), NOT as a
 * second stacked row — the kit toolbar wraps this in a `flex items-center gap-2`,
 * so this returns the controls as a fragment (no row wrapper / margins of its own).
 *
 * Copy / Export are intentionally NOT rendered here — the file-viewer header
 * (viewers/tabular/header.tsx → DelimitedHeader) owns the view-aware Export +
 * Copy-selection affordances, driven off the snapshot the body publishes into
 * `FileStore.fileTabularView`. The `onCopy`/`onExport`/`exportLabel` props are
 * retained (optional, unused here) so callers with a body-local toolbar can
 * still wire them without a churny signature change.
 */
export function TabularToolbar({
  testidPrefix,
  total,
  viewCount,
  onJump,
}: {
  testidPrefix: string
  /** Total parsed rows (pre-filter). */
  total: number
  /** Rows currently in view (post-filter). */
  viewCount: number
  /** Called with a 1-based ORIGINAL row number to scroll into view. */
  onJump: (rowNumber: number) => void
  onCopy?: () => void
  onExport?: () => void
  exportLabel?: string
}) {
  const [jump, setJump] = useState<number | null>(null)
  const filtered = viewCount !== total
  const noun = (n: number) => (n === 1 ? 'row' : 'rows')
  return (
    <Fragment>
      <Text type="secondary" className="text-xs whitespace-nowrap" data-testid={`${testidPrefix}-readout`}>
        {filtered
          ? `Showing ${viewCount.toLocaleString()} of ${total.toLocaleString()} ${noun(total)}`
          : `${total.toLocaleString()} ${noun(total)}`}
      </Text>
      <div className="flex items-center gap-1">
        <Text type="secondary" className="text-xs whitespace-nowrap">
          Jump to row
        </Text>
        <InputNumber
          // size="sm" matches the sibling filter Input (also size="sm") sharing
          // this toolbar row.
          size="sm"
          data-standalone-control
          data-testid={`${testidPrefix}-jump-input`}
          aria-label="Jump to row number"
          min={1}
          max={total}
          value={jump}
          onChange={v => setJump(typeof v === 'number' ? v : null)}
          onKeyDown={e => { if (e.key === 'Enter' && jump != null) onJump(jump) }}
          className="w-24"
        />
        <Button
          size="default"
          variant="outline"
          data-testid={`${testidPrefix}-jump-apply`}
          onClick={() => jump != null && onJump(jump)}
        >
          Go
        </Button>
      </div>
    </Fragment>
  )
}
