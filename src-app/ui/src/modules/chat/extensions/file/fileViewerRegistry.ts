import type { FileViewerModule, FileViewerEntry, FileSupportEntry } from './types'

type ViewerModule = { viewers: FileViewerModule[] }

const modules = import.meta.glob<ViewerModule>('./file-viewers/*/module.tsx', { eager: true })
const allViewers: FileViewerModule[] = Object.values(modules).flatMap(m => m.viewers)

/**
 * Score a single support rule against a file. Returns the rule's `priority`
 * (default 0) when it matches, or `null` when it doesn't. Lower score = more
 * specific = preferred in `getViewer`.
 *
 * Matching is OR across `ext` and `mime` within a single rule — having both
 * fields means "either matches counts as a match". `mime` supports a trailing
 * `/*` wildcard to match any subtype of a top-level type.
 */
function scoreSupport(
  rule: FileSupportEntry,
  filename: string,
  mimeType?: string,
): number | null {
  if (rule.ext !== undefined) {
    const ext = filename.split('.').pop()?.toLowerCase()
    if (ext === rule.ext.toLowerCase()) return rule.priority ?? 0
  }
  if (rule.mime !== undefined && mimeType) {
    if (rule.mime.endsWith('/*')) {
      const prefix = rule.mime.slice(0, -1)
      if (mimeType.startsWith(prefix)) return rule.priority ?? 0
    } else if (mimeType.toLowerCase() === rule.mime.toLowerCase()) {
      return rule.priority ?? 0
    }
  }
  return null
}

/**
 * Resolve the best viewer for a file by scanning every registered viewer's
 * `supportedTypes` and picking the viewer whose best-matching rule has the
 * lowest specificity score. Ties go to the first encountered (filesystem
 * alphabetical iteration of modules), which is rare in practice — explicit
 * priorities should be used to break intentional overlaps.
 */
export function getViewer(filename: string, mimeType?: string): FileViewerEntry | undefined {
  let best: { entry: FileViewerEntry; score: number } | null = null
  for (const viewer of allViewers) {
    let viewerBest: number | null = null
    for (const rule of viewer.supportedTypes) {
      const s = scoreSupport(rule, filename, mimeType)
      if (s === null) continue
      if (viewerBest === null || s < viewerBest) viewerBest = s
    }
    if (viewerBest === null) continue
    if (best === null || viewerBest < best.score) {
      best = { entry: viewer.entry, score: viewerBest }
    }
  }
  return best?.entry
}
