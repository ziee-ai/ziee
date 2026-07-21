import { useMemo } from 'react'
import { Spin } from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'
import { lineDiff } from '@/modules/file/utils/lineDiff'

/**
 * Renders a line-level added/removed diff between two versions of a file. Text
 * is fetched (+ cached) via the FileVersions store; reads `versionTextCache`
 * reactively so it re-renders when the async loads land.
 */
export function FileVersionDiff({
  fileId,
  from,
  to,
}: {
  fileId: string
  from: number
  to: number
}) {
  const cache = Stores.FileVersions.versionTextCache
  // Fire-and-forget background loads if not already loaded/cached.
  if (cache.get(`${fileId}:${from}`) === undefined) {
    void Stores.FileVersions.loadVersionText(fileId, from)
  }
  if (cache.get(`${fileId}:${to}`) === undefined) {
    void Stores.FileVersions.loadVersionText(fileId, to)
  }
  const a = cache.get(`${fileId}:${from}`) ?? null
  const b = cache.get(`${fileId}:${to}`) ?? null
  const lines = useMemo(
    () => (a != null && b != null ? lineDiff(a, b) : []),
    [a, b],
  )

  if (a == null || b == null) {
    return (
      <div className="flex h-48 items-center justify-center">
        <Spin label="Loading diff" />
      </div>
    )
  }

  return (
    <div
      className="max-h-[60vh] overflow-auto rounded-md border border-border font-mono text-xs"
      data-testid="file-version-diff"
    >
      {lines.map((l, i) => (
        <div
          key={i}
          className={`whitespace-pre-wrap px-2 ${
            l.type === 'add'
              ? 'bg-success/10 text-success'
              : l.type === 'del'
                ? 'bg-destructive/10 text-destructive'
                : 'text-muted-foreground'
          }`}
        >
          <span className="select-none pe-2 opacity-60">
            {l.type === 'add' ? '+' : l.type === 'del' ? '-' : ' '}
          </span>
          {l.text || ' '}
        </div>
      ))}
    </div>
  )
}
