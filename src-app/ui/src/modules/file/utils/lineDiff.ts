import { diffLines } from 'diff'

export type DiffLineType = 'add' | 'del' | 'ctx'

export interface DiffLine {
  type: DiffLineType
  text: string
}

/**
 * Line-level diff of two text versions (for the version-diff view). Wraps
 * jsdiff's `diffLines` and flattens it into per-line entries so the UI can render
 * added / removed / context lines. Pure — unit-tested.
 */
export function lineDiff(a: string, b: string): DiffLine[] {
  const parts = diffLines(a ?? '', b ?? '')
  const out: DiffLine[] = []
  for (const p of parts) {
    const type: DiffLineType = p.added ? 'add' : p.removed ? 'del' : 'ctx'
    const lines = p.value.split('\n')
    // jsdiff values end with a trailing newline → a final '' element; drop it.
    if (lines.length > 0 && lines[lines.length - 1] === '') lines.pop()
    for (const line of lines) out.push({ type, text: line })
  }
  return out
}
