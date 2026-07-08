import { ClipboardCopy, FileDown } from 'lucide-react'
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
// `DownloadButton` (hence the FileDown glyph, not the shell's Download tray).
// Disabled until the body has published a snapshot.
function TabularViewActions({ file }: { file: FileEntity }) {
  // Reactive read (re-enables + retitles when the body publishes); handlers
  // re-read the raw snapshot via `$` at click time so they act on the latest view.
  const view = Stores.File.fileTabularView.get(file.id)
  const hasView = !!view
  // Name the delimited format the download will use (mirrors the removed
  // toolbar's "Export CSV"/"Export TSV" labels).
  const exportLabel = view?.delimiter === '\t' ? 'Export view (TSV)' : 'Export view (CSV)'
  const onCopySelection = async () => {
    const v = Stores.File.$.fileTabularView.get(file.id)
    if (!v) return
    // Mirror chrome.tsx's CopySelectionButton: warn (don't clobber the clipboard)
    // when nothing is selected, rather than silently copying the whole view.
    if (!v.selectionTsv) {
      message.warning('Select a cell to copy')
      return
    }
    if (await copyTabularSelection(v)) message.success('Copied selection')
    else message.error('Failed to copy')
  }
  const onExport = () => {
    const v = Stores.File.$.fileTabularView.get(file.id)
    if (!v) return
    try {
      exportTabularView(v)
    } catch {
      message.error('Failed to export')
    }
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
        data-testid="file-viewer-tabular-copy-selection-btn"
      />
      <Button
        variant="ghost"
        size="icon"
        tooltip={exportLabel}
        aria-label={exportLabel}
        icon={<FileDown />}
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
