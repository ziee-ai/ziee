import { FileQuestion, Pencil, TriangleAlert } from 'lucide-react'
import { useState, useEffect } from 'react'
import { Button, Empty, Spin, Text, Title } from '@ziee/kit'
import type { File as FileEntity } from '@/api-client/types'
import { getViewer } from '@/modules/file/registry/fileViewerRegistry'
import {
  DownloadButton,
  FullPageButton,
} from '@/modules/file/viewers/shared/chrome'
import { FileVersionBar } from '@/modules/file/components/FileVersionBar'
import { FileEditBody } from '@/modules/file/components/FileEditBody'
import { FileExportMenu } from '@/modules/file/components/FileExportMenu'
import { DeliverablePinButton } from '@/modules/file/components/DeliverablePinButton'
import { editableKind } from '@/modules/file/utils/editableTypes'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

/** Hard cap on previewable file size — the SINGLE outer OOM backstop that
 *  prevents even fetching a pathological file. Files above this never trigger a
 *  content download — the panel renders a "too large to preview" empty state
 *  instead. The cutoff intentionally covers the common log/dataset/spreadsheet
 *  "kinda big" range (most files <1 MB; large CSVs / parquets / logs commonly
 *  exceed 50 MB). Above 10 MB the cost of fetching + parsing + rendering starts
 *  hurting more than the preview is worth — Download is still one click away.
 *
 *  After file-viewer-virtualization the viewers virtualize/window their render
 *  (text chunk-on-demand highlight; tabular row-virtualization), so their
 *  per-viewer caps (RAWCODE_MAX_LINES / DELIMITED_MAX_ROWS / XLSX_MAX_ROWS) are
 *  now high OOM GUARDS for the "many tiny lines/rows" case that a byte bound
 *  can't catch — NOT preview-truncation UX. This 10 MB byte cap remains the one
 *  upstream bound and is deliberately left unchanged (raising it would enlarge
 *  the memory-heavy paths: whole-file DOM, in-memory dataSource, xlsx decompress). */
const PREVIEW_SIZE_LIMIT_BYTES = 10 * 1024 * 1024

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

interface FilePanelProps {
  file: FileEntity
  /** When true, skip the internal title bar — the host (e.g.
   *  FilePreviewDrawer) renders filename + actions in its own header
   *  slot instead. Defaults to false so chat's right-panel surface
   *  keeps the existing title bar. */
  hideHeader?: boolean
  /** Open the panel pinned to a specific version (e.g. a message attachment
   *  pinned to the version that existed when it was sent). `undefined`/head =
   *  show the latest. */
  initialVersion?: number
  /** Forwarded to the header actions — hide "Open full page" (set by the
   *  full-page view, which is already at that route). */
  showFullPage?: boolean
}

/**
 * Renders the right-side actions for a file (viewer-specific
 * HeaderActions if the matched viewer declares them, otherwise a
 * Download button). Exported so hosts that render their own title bar
 * (FilePreviewDrawer) can place these actions next to the filename
 * without duplicating the viewer-registry lookup.
 */
export function FilePanelHeaderActions({
  file,
  showFullPage = true,
}: {
  file: FileEntity
  /** Hide the "Open full page" button — set on the full-page view itself, where
   *  it would navigate to the same route. Open-in-new-tab stays (still useful). */
  showFullPage?: boolean
}) {
  const handler = getViewer(file.filename, file.mime_type ?? undefined)
  const HeaderActions = handler?.headerActions
  return (
    <>
      {/* Viewer-specific chrome (toggles / copy / zoom …), when the matched
          viewer declares any. */}
      {HeaderActions ? <HeaderActions file={file} /> : null}
      {/* Export-as (format conversion) for text deliverables — distinct from the
          plain Download of original bytes below. */}
      {editableKind(file) === 'markdown' ? <FileExportMenu file={file} /> : null}
      {/* Pin/unpin as a deliverable of the active conversation (no-op outside one). */}
      <DeliverablePinButton file={file} />
      {/* Download is a shell-level affordance guaranteed for EVERY file type. */}
      <DownloadButton file={file} />
      {showFullPage ? <FullPageButton file={file} /> : null}
    </>
  )
}

/**
 * Panel shell — owns the title bar and overall layout, delegates everything
 * inside the panel body (and the action area to the right of the title) to
 * the matching viewer's `body` and optional `headerActions` slot components.
 */
export function FilePanel({ file, hideHeader = false, initialVersion, showFullPage = true }: FilePanelProps) {
  const handler = getViewer(file.filename, file.mime_type ?? undefined)
  const Body = handler?.body
  const tooLarge = file.file_size > PREVIEW_SIZE_LIMIT_BYTES

  // Version-viewing state: null = head (normal viewer body); a number = view
  // that version's content read-only.
  const [selectedVersion, setSelectedVersion] = useState<number | null>(
    initialVersion && initialVersion !== file.version ? initialVersion : null,
  )
  // Re-sync when the panel is reused for a different pinned version / file (the
  // right-panel keys tabs by file id, so the same tab can be reopened pinned to
  // a different version). useState's initializer only runs on first mount.
  useEffect(() => {
    setSelectedVersion(
      initialVersion && initialVersion !== file.version ? initialVersion : null,
    )
  }, [initialVersion, file.version, file.id])
  // Canvas edit mode — only offered for editable text types (markdown in v1) at
  // the head version. Entering edit replaces the read-only viewer body.
  const [editing, setEditing] = useState(false)
  // Editing appends a new file version (a `files::upload` mutation), so the Edit
  // affordance is gated on FilesUpload — a user without it must never reach the
  // editable canvas (FileEditBody / CsvGridEditor), not merely 403 on Save. This
  // mirrors the module's affordance-gating convention (FileCard's canDownload,
  // FileUploadButton's canUpload).
  const canUpload = usePermission(Permissions.FilesUpload)
  const canEdit = editableKind(file) !== null && !tooLarge && canUpload
  // Exit edit mode when the panel is reused for a DIFFERENT file (the global
  // FilePreviewDrawer swaps the `file` prop without remounting FilePanel). Without
  // this, a stale editor could Save one file's content onto another. Belt-and-
  // suspenders with the `key={file.id}` on FileEditBody below (fresh remount).
  useEffect(() => {
    setEditing(false)
  }, [file.id])
  const isViewingOld = selectedVersion !== null && selectedVersion !== file.version
  // Read versionTextCache REACTIVELY so the body re-renders when the async text
  // load lands. getVersionText() reads via getState() + kicks off the load but
  // does NOT subscribe (same reason FileVersionBar reads versionsByFile).
  const versionTextCache = Stores.FileVersions.versionTextCache
  const oldVersionText = isViewingOld
    ? versionTextCache.get(`${file.id}:${selectedVersion}`) ??
      Stores.FileVersions.getVersionText(file.id, selectedVersion as number)
    : null

  return (
    <div className="flex flex-col h-full w-full bg-card">
      {/* Title bar — panel-owned. Viewer fills the right-side actions area
          when there's a registered viewer; otherwise we surface Download.
          Hosts that render their own header (FilePreviewDrawer) pass
          hideHeader to skip this and avoid duplication. */}
      {!hideHeader && (
        <div
          // bg-muted: a muted header band distinct from the bg-card panel body
          // (which matches the app shell / tab strip). matches the drawer footer /
          // find-bar convention.
          className="flex items-center gap-2 px-3 py-2 flex-shrink-0 border-border border-b bg-muted"
        >
          <Title level={5} className="!m-0 flex-1 truncate" title={file.filename}>
            {file.filename}
          </Title>
          {/* Too-large files always get plain Download — viewer-specific
              actions (PDF page nav, CSV controls, etc.) need the body
              loaded to be meaningful. */}
          {canEdit && !editing && !isViewingOld && (
            <Button
              variant="ghost"
              onClick={() => setEditing(true)}
              data-testid="canvas-edit-toggle"
              aria-label="Edit"
            >
              <Pencil className="size-3.5" />
              Edit
            </Button>
          )}
          {tooLarge ? <DownloadButton file={file} /> : <FilePanelHeaderActions file={file} showFullPage={showFullPage} />}
        </div>
      )}

      {/* Version history + restore — only shown once a file has >1 version. */}
      <FileVersionBar
        file={file}
        selectedVersion={selectedVersion}
        onSelectVersion={setSelectedVersion}
      />

      {/* Body — fully owned by the viewer unless the file exceeds the
          preview-size cap (we skip loading entirely) or no viewer
          matches. When viewing a non-head version, render that version's
          text read-only instead (the viewers are head-bound). */}
      <div className="flex-1 overflow-hidden bg-card">
        {editing
          ? <FileEditBody key={file.id} file={file} onDone={() => setEditing(false)} />
          : isViewingOld
          ? (
            oldVersionText === null
              ? (
                <div className="flex items-center justify-center h-full">
                  <Spin label="Loading" />
                </div>
              )
              : (
                <pre
                  className="h-full w-full overflow-auto m-0 p-3 text-xs whitespace-pre-wrap break-words"
                  data-testid="file-version-readonly-body"
                >
                  {oldVersionText}
                </pre>
              )
          )
          : tooLarge
          ? (
            <div
              className="flex flex-col items-center justify-center h-full p-6"
              data-testid="too-large-to-preview"
            >
              <Empty
                data-testid="file-panel-too-large-empty"
                icon={<TriangleAlert className="text-5xl text-warning" />}
                description={
                  <div className="flex flex-col items-center gap-1">
                    <Text strong>File too large to preview</Text>
                    <Text type="secondary" className="text-xs">
                      {file.filename} is{' '}
                      <Text code className="!text-xs">{formatBytes(file.file_size)}</Text>
                      , above the{' '}
                      <Text code className="!text-xs">{formatBytes(PREVIEW_SIZE_LIMIT_BYTES)}</Text>
                      {' '}preview limit. Use the download button to open the original.
                    </Text>
                  </div>
                }
              />
            </div>
          )
          : Body
          ? <Body file={file} />
          : (
            <div
              className="flex flex-col items-center justify-center h-full p-6"
              data-testid="cannot-preview"
            >
              <Empty
                data-testid="file-panel-cannot-preview-empty"
                icon={<FileQuestion className="text-5xl text-muted-foreground" />}
                description={
                  <div className="flex flex-col items-center gap-1">
                    <Text strong>Cannot preview this file</Text>
                    <Text type="secondary" className="text-xs">
                      No viewer is registered for{' '}
                      <Text code className="!text-xs">
                        {file.mime_type || file.filename.split('.').pop() || 'this file type'}
                      </Text>
                      . Use the download button above to open the original.
                    </Text>
                  </div>
                }
              />
            </div>
          )}
      </div>
    </div>
  )
}
