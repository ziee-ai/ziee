import { Button, Popover } from '@ziee/kit'

/** Above this length a cell is treated as "clippable" and gets a click-to-expand
 *  popover in addition to the truncation + hover title (ITEM-16 / DEC-11). */
export const EXPAND_THRESHOLD = 40

/**
 * A tabular data cell: single-line truncation + a native `title` hover tooltip,
 * and — when the value is long enough to clip — a click that opens a popover
 * showing the full value. The click also bubbles to the enclosing table cell so
 * the kit Table's cell-selection still fires (both are "focus this cell").
 */
export function ExpandableCell({ value, testid }: { value: string; testid: string }) {
  if (value.length <= EXPAND_THRESHOLD) {
    return (
      <span className="block truncate" title={value || undefined}>
        {value}
      </span>
    )
  }
  return (
    <Popover
      content={
        <div
          className="max-w-md max-h-64 overflow-auto whitespace-pre-wrap break-words text-sm p-1"
          data-testid={`${testid}-popover`}
        >
          {value}
        </div>
      }
    >
      <Button
        variant="ghost"
        title={value}
        data-testid={testid}
        className="block w-full max-w-full truncate text-start px-0 h-auto min-h-6 py-0 font-normal hover:underline underline-offset-2"
      >
        {value}
      </Button>
    </Popover>
  )
}
