import { useState } from 'react'
import { Copy, Download } from 'lucide-react'
import { Button, InputNumber, Text } from '@/components/ui'

/**
 * Body-local toolbar for the tabular viewer (DEC-15): a "row X of Y" readout, a
 * jump-to-row input, and Copy + Export-view actions. Rendered ABOVE the kit
 * Table (which owns its own search + column-chooser toolbar).
 */
export function TabularToolbar({
  testidPrefix,
  total,
  viewCount,
  onJump,
  onCopy,
  onExport,
  exportLabel,
}: {
  testidPrefix: string
  /** Total parsed rows (pre-filter). */
  total: number
  /** Rows currently in view (post-filter). */
  viewCount: number
  /** Called with a 1-based ORIGINAL row number to scroll into view. */
  onJump: (rowNumber: number) => void
  onCopy: () => void
  onExport: () => void
  exportLabel: string
}) {
  const [jump, setJump] = useState<number | null>(null)
  const filtered = viewCount !== total
  const noun = (n: number) => (n === 1 ? 'row' : 'rows')
  return (
    <div className="mb-2 flex flex-wrap items-center gap-2" data-testid={`${testidPrefix}-toolbar`}>
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
      <div className="ms-auto flex items-center gap-1">
        <Button
          variant="outline"
          icon={<Copy />}
          data-testid={`${testidPrefix}-copy`}
          onClick={onCopy}
        >
          Copy
        </Button>
        <Button
          variant="outline"
          icon={<Download />}
          data-testid={`${testidPrefix}-export`}
          onClick={onExport}
        >
          {exportLabel}
        </Button>
      </div>
    </div>
  )
}
