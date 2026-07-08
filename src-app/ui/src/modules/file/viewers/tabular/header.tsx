import { ClipboardCopy, Download } from 'lucide-react'
import { Button, Space, message } from '@/components/ui'
import { Stores } from '@/core/stores'
import type { File as FileEntity } from '@/api-client/types'
import { CopyButton, RawToggle } from '../shared/chrome'
import { copyTabularSelection, exportTabularView } from './tableView'
import type { FileViewerSlotProps } from '../../types/viewer'

// ── TabularViewActions ───────────────────────────────────────────────────────
// The view-aware Export + Copy-selection affordances for the CSV/TSV header.
// They act on the CURRENT view (filtered/sorted rows, visible columns, cell
// selection) that the body publishes into `FileStore.fileTabularView` — distinct
// from the header's whole-file `CopyButton` and the shell's original-bytes
// `DownloadButton`. Disabled until the body has published a snapshot.
function TabularViewActions({ file }: { file: FileEntity }) {
  // Reactive read (re-enables when the body publishes); handlers re-read the
  // raw snapshot via `$` at click time so they act on the latest view.
  const hasView = Stores.File.fileTabularView.has(file.id)
  const onCopySelection = async () => {
    const view = Stores.File.$.fileTabularView.get(file.id)
    if (!view) return
    if (await copyTabularSelection(view)) message.success('Copied to clipboard')
    else message.error('Failed to copy')
  }
  const onExport = () => {
    const view = Stores.File.$.fileTabularView.get(file.id)
    if (view) exportTabularView(view)
  }
  return (
    <>
      <Button
        variant="ghost"
        size="icon"
        tooltip="Copy selection"
        aria-label="Copy selection"
        icon={<ClipboardCopy />}
        disabled={!hasView}
        onClick={onCopySelection}
        data-testid="file-viewer-tabular-copy-btn"
      />
      <Button
        variant="ghost"
        size="icon"
        tooltip="Export view"
        aria-label="Export view"
        icon={<Download />}
        disabled={!hasView}
        onClick={onExport}
        data-testid="file-viewer-tabular-export-btn"
      />
    </>
  )
}

/** CSV/TSV header — raw toggle + whole-file copy + view-aware Export /
 *  Copy-selection. (Original-bytes Download is a shell-level affordance rendered
 *  by the host — InlineFilePreview / FilePanelHeaderActions.) */
export function DelimitedHeader(props: FileViewerSlotProps) {
  if (!('file' in props)) return null
  const { file } = props
  return (
    <Space size="small" wrap={false}>
      <RawToggle file={file} />
      <CopyButton file={file} />
      <TabularViewActions file={file} />
    </Space>
  )
}

/** XLSX header — binary format, no viewer-specific chrome (the host renders the
 *  shared Download affordance). */
export function XlsxHeader(_props: FileViewerSlotProps) {
  return null
}
