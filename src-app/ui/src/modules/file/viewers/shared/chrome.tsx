import { App, Button, Space } from 'antd'
import { CodeOutlined, DownloadOutlined, EyeOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { File as FileEntity } from '@/api-client/types'

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
    <Space.Compact>
      <Button
        icon={<EyeOutlined />}
        type={mode === 'compiled' ? 'primary' : 'default'}
        title="Rendered view"
        aria-label="Rendered view"
        onClick={() => Stores.File.setFileViewMode(file.id, 'compiled')}
      />
      <Button
        icon={<CodeOutlined />}
        type={mode === 'raw' ? 'primary' : 'default'}
        title="Raw view"
        aria-label="Raw view"
        onClick={() => Stores.File.setFileViewMode(file.id, 'raw')}
      />
    </Space.Compact>
  )
}

// ── CopyButton ──────────────────────────────────────────────────────────────
// Copies the file's text contents to clipboard. Assumes the viewer has
// already triggered (or will trigger) text-content loading via FileStore.
// Useful for any text-based viewer.

export function CopyButton({ file }: { file: FileEntity }) {
  const { message } = App.useApp()
  const handleCopy = async () => {
    const text = Stores.File.fileTextContents.get(file.id) ?? ''
    if (!text) {
      message.warning('Nothing to copy yet — content still loading')
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
    <Button style={{ fontSize: 15 }} onClick={handleCopy}>
      Copy
    </Button>
  )
}

// ── DownloadButton ──────────────────────────────────────────────────────────
// Triggers a download of the original file. Works for any file type since
// it just streams the original bytes from the server.

export function DownloadButton({ file }: { file: FileEntity }) {
  const { message } = App.useApp()
  return (
    <Button
      icon={<DownloadOutlined />}
      onClick={() => {
        Stores.File.downloadFile(file).catch(() =>
          message.error('Failed to download file'),
        )
      }}
    >
      Download
    </Button>
  )
}
