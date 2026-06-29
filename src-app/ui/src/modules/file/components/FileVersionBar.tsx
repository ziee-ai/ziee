import { History, Undo2 } from 'lucide-react'
import { useState } from 'react'
import { Select, Button, Tag } from '@/components/ui'
import { message } from '@/components/ui'
import { Stores } from '@/core/stores'
import type { File as FileEntity } from '@/api-client/types'

interface FileVersionBarProps {
  file: FileEntity
  /** Currently-viewed version (`null` = head). */
  selectedVersion: number | null
  onSelectVersion: (version: number | null) => void
}

function relTime(iso: string): string {
  if (!iso) return ''
  const diff = Date.now() - new Date(iso).getTime()
  const m = Math.floor(diff / 60000)
  if (m < 1) return 'just now'
  if (m < 60) return `${m}m ago`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h}h ago`
  return `${Math.floor(h / 24)}d ago`
}

/**
 * Version history + restore bar shown above a file's panel body. Renders
 * nothing for single-version files (the common case) so it stays out of the
 * way. Selecting a non-head version puts the panel into read-only "viewing an
 * old version" mode; Restore appends a new head equal to that version
 * (append-only — prior versions are never lost).
 */
export function FileVersionBar({ file, selectedVersion, onSelectVersion }: FileVersionBarProps) {
  // Read `versionsByFile` REACTIVELY so the bar re-renders when the async
  // version load lands. `getVersions()` reads via getState() + kicks off that
  // load (render-safe) but does NOT subscribe — without touching the reactive
  // map here, the bar would render once (empty) and never update.
  const versionsByFile = Stores.FileVersions.versionsByFile
  const versions = versionsByFile.get(file.id) ?? Stores.FileVersions.getVersions(file.id)
  const [restoring, setRestoring] = useState(false)

  if (versions.length <= 1) return null

  const headVersion = file.version
  const current = selectedVersion ?? headVersion
  const isViewingOld = current !== headVersion

  const handleRestore = async () => {
    setRestoring(true)
    try {
      // `__state` (not the render-only `Stores.X` proxy) for store access from
      // an event handler — the proxy fires React hooks on every access and would
      // be a Rules-of-Hooks violation outside render.
      await Stores.FileVersions.__state.restoreVersion(file.id, current)
      onSelectVersion(null)
    } catch (e) {
      message.error(`Failed to restore v${current}`)
      console.error('[FileVersionBar] restore failed', e)
    } finally {
      setRestoring(false)
    }
  }

  return (
    <div
      className="flex items-center gap-2 px-3 py-1.5 flex-shrink-0 flex-wrap border-b border-border bg-muted/40"
      data-testid="file-version-bar"
    >
      <History className="text-muted-foreground" />
      <Select
        size="sm"
        value={String(current)}
        className="min-w-[220px]"
        onChange={(v) => onSelectVersion(v === String(headVersion) ? null : Number(v))}
        options={versions.map((ver) => ({
          value: String(ver.version),
          label: `v${ver.version}${ver.version === headVersion ? ' (current)' : ''} · ${relTime(ver.created_at)} · ${ver.created_by}`,
        }))}
        data-testid="file-version-select"
      />
      {isViewingOld ? (
        <>
          <Tag tone="warning" data-testid="file-version-old-tag">viewing v{current} — not current</Tag>
          <Button
            size="sm"
            variant="outline"
            icon={<Undo2 />}
            loading={restoring}
            onClick={handleRestore}
            data-testid="file-version-restore"
          >
            Restore this version
          </Button>
          <Button size="sm" variant="outline" onClick={() => onSelectVersion(null)} data-testid="file-version-back-latest">
            Back to latest
          </Button>
        </>
      ) : (
        <Tag data-testid="file-version-current-tag">v{headVersion} · {versions.length} versions</Tag>
      )}
    </div>
  )
}
