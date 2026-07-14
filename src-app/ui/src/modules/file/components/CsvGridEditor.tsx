import { forwardRef, useImperativeHandle, useState } from 'react'
import { Plus, Trash2 } from 'lucide-react'
import { Button, Input } from '@ziee/kit'
import type { CanvasEditorHandle } from '@/components/kit/editor/types'
import {
  type CsvGrid,
  parseCsv,
  serializeCsv,
} from '@/modules/file/utils/csvRoundtrip'

interface CsvGridEditorProps {
  initialText: string
  onDirty?: () => void
}

/**
 * Editable spreadsheet-style grid for `csv` deliverables. Parses the full CSV
 * (no row cap → no data-loss on save), edits cells/headers/rows in place, and
 * serializes back to RFC-4180 CSV on Save via the shared imperative handle.
 */
export const CsvGridEditor = forwardRef<CanvasEditorHandle, CsvGridEditorProps>(
  function CsvGridEditor({ initialText, onDirty }, ref) {
    const [grid, setGrid] = useState<CsvGrid>(() => parseCsv(initialText))
    useImperativeHandle(ref, () => ({ getContent: () => serializeCsv(grid) }), [
      grid,
    ])

    const setHeader = (c: number, v: string) => {
      setGrid(g => {
        const headers = g.headers.slice()
        headers[c] = v
        return { ...g, headers }
      })
      onDirty?.()
    }
    const setCell = (r: number, c: number, v: string) => {
      setGrid(g => {
        // Clone only the array + the edited row (O(cols)), not the whole matrix
        // (O(rows×cols)) — the latter froze large CSVs on every keystroke.
        const rows = g.rows.slice()
        const row = rows[r].slice()
        while (row.length <= c) row.push('')
        row[c] = v
        rows[r] = row
        return { ...g, rows }
      })
      onDirty?.()
    }
    const addRow = () => {
      setGrid(g => ({ ...g, rows: [...g.rows, g.headers.map(() => '')] }))
      onDirty?.()
    }
    const deleteRow = (r: number) => {
      setGrid(g => ({ ...g, rows: g.rows.filter((_, i) => i !== r) }))
      onDirty?.()
    }

    return (
      <div className="h-full overflow-auto p-2" data-testid="canvas-csv-grid">
        <table className="w-full border-collapse text-sm">
          <thead>
            <tr>
              <th className="w-8" />
              {grid.headers.map((h, c) => (
                <th key={c} className="border border-border p-0">
                  <Input
                    aria-label={`Column ${c + 1} header`}
                    data-testid={`csv-header-${c}`}
                    value={h}
                    onChange={e => setHeader(c, e.target.value)}
                    className="border-0 font-semibold"
                  />
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {grid.rows.map((row, r) => (
              <tr key={r}>
                <td className="text-center align-middle">
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={`Delete row ${r + 1}`}
                    data-testid={`csv-delrow-${r}`}
                    onClick={() => deleteRow(r)}
                  >
                    <Trash2 className="size-3.5" />
                  </Button>
                </td>
                {grid.headers.map((_, c) => (
                  <td key={c} className="border border-border p-0">
                    <Input
                      aria-label={`Row ${r + 1} column ${c + 1}`}
                      data-testid={`csv-cell-${r}-${c}`}
                      value={row[c] ?? ''}
                      onChange={e => setCell(r, c, e.target.value)}
                      className="border-0"
                    />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
        <Button
          variant="outline"
          size="default"
          className="mt-2"
          onClick={addRow}
          data-testid="csv-add-row"
        >
          <Plus className="size-3.5" /> Add row
        </Button>
      </div>
    )
  },
)
