import { Code, Copy, Download, Eye } from 'lucide-react'
import { Button, Segmented, Tooltip } from '@/components/ui'
import { Stores } from '@/core/stores'
import type { File as FileEntity } from '@/api-client/types'
import { message } from '@/components/ui'

/**
 * Shared chrome building blocks for file viewer headers. Viewers compose
 * these in their `headerActions` slot. Each component reads/writes the
 * FileStore directly — header buttons and body renderers are sibling
 * components, so the store is the only sane shared-state surface.
 */

// ── RawToggle ───────────────────────────────────────────────────────────────
// For viewers with both a rendered form and a meaningful raw-text view
// (markdown, csv/tsv, html, svg). Pairs with body code that reads the same
// fileViewModes entry to decide which form to render.
//
// Renders nothing when the backend hasn't extracted text for this file
// (`text_page_count === 0`). Without this guard, a misconfigured viewer
// could expose the toggle on a binary file and silently switch the body
// to an empty RawCodeView. Sourcing the gate from a known backend field
// (rather than the viewer author declaring `compilable: true/false`)
// keeps the architecture self-correcting — new viewers can't get this
// wrong, and a file type that gains text extraction server-side
// automatically becomes toggleable client-side.

export function RawToggle({ file }: { file: FileEntity }) {
  if (file.text_page_count === 0) return null
  const mode = Stores.File.fileViewModes.get(file.id) ?? 'compiled'
  return (
    <Segmented
      value={mode}
      onChange={(v: string) =>
        Stores.File.setFileViewMode(file.id, v as 'compiled' | 'raw')
      }
      data-testid="file-viewer-mode-segmented"
      options={[
        {
          value: 'compiled',
          label: (
            <Tooltip title="Rendered view">
              <span
                className="flex items-center"
                aria-label="Rendered view"
                data-testid="file-viewer-rendered-btn"
              >
                <Eye />
              </span>
            </Tooltip>
          ),
        },
        {
          value: 'raw',
          label: (
            <Tooltip title="Raw view">
              <span
                className="flex items-center"
                aria-label="Raw view"
                data-testid="file-viewer-raw-btn"
              >
                <Code />
              </span>
            </Tooltip>
          ),
        },
      ]}
    />
  )
}

// ── CopyButton ──────────────────────────────────────────────────────────────
// Copies the file's text contents to clipboard. Assumes the viewer has
// already triggered (or will trigger) text-content loading via FileStore.
// Useful for any text-based viewer.

export function CopyButton({ file }: { file: FileEntity }) {
  const handleCopy = async () => {
    // `Stores.File.__state` is the raw zustand getState — bypasses
    // the reactive proxy, safe to use inside event handlers. The
    // proxy's `.get` trap calls useEffect/useStore on every
    // property access (see core/stores.ts:266), which is a Rules-
    // of-Hooks violation when triggered outside render. Falls back
    // to driving the load ourselves if the cache is cold, so Copy
    // works even when the user clicks before the body finishes its
    // async fetch.
    // `undefined` = not loaded, or a prior load FAILED — File.store doesn't
    // cache an error sentinel, it just leaves the entry absent. So a cold/failed
    // read drives the load itself and re-reads (a retry on each click).
    let text = Stores.File.__state.fileTextContents.get(file.id)
    if (text === undefined) {
      await Stores.File.__state.loadFileTextContent(file.id, file)
      text = Stores.File.__state.fileTextContents.get(file.id)
    }
    if (text === undefined || text === '') {
      message.error('Failed to load file content')
      return
    }
    try {
      await navigator.clipboard.writeText(text)
      message.success('Copied to clipboard')
    } catch {
      message.error('Failed to copy')
    }
  }
  return (
    // ghost, matching the drawer's own close affordance — peer icon-only header
    // actions share ONE variant (Spec B) rather than mixing outline + ghost.
    <Button
      variant="ghost"
      size="icon"
      tooltip="Copy"
      // Default (top) placement so ALL file-viewer header actions share one
      // tooltip side; Base UI's collision handling flips it to the bottom only
      // when it would actually clip at the panel's top edge.
      icon={<Copy />}
      onClick={handleCopy}
      data-testid="file-viewer-copy-btn"
    />
  )
}

// ── DownloadButton ──────────────────────────────────────────────────────────
// Triggers a download of the original file. Works for any file type since
// it just streams the original bytes from the server.

export function DownloadButton({ file }: { file: FileEntity }) {
  return (
    // ghost, matching the drawer's close affordance + the Copy button — peer
    // icon-only header actions share ONE variant (Spec B).
    <Button
      variant="ghost"
      size="icon"
      tooltip="Download"
      // See CopyButton: default (top) placement to match every other header
      // action; Base UI flips it to the bottom only if it would clip.
      icon={<Download />}
      onClick={() => {
        Stores.File.downloadFile(file).catch(() =>
          message.error('Failed to download file'),
        )
      }}
      data-testid="file-viewer-download-btn"
    />
  )
}
