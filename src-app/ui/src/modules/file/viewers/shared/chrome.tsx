import {
  ClipboardCopy,
  Code,
  Copy,
  Download,
  ExternalLink,
  Eye,
  Maximize2,
  Search,
  WrapText,
} from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { Button, Segmented, Tooltip } from '@/components/ui'
import { Stores } from '@/core/stores'
import type { File as FileEntity } from '@/api-client/types'
import { message } from '@/components/ui'
import { isHighlightSupported } from './find/highlightSupported'

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
          // aria-label on the option names the interactive tab itself (the inner
          // icon span's label doesn't propagate up to the role=tab element).
          'aria-label': 'Rendered view',
          label: (
            <Tooltip title="Rendered view">
              <span
                className="flex items-center"
                data-testid="file-viewer-rendered-btn"
              >
                <Eye />
              </span>
            </Tooltip>
          ),
        },
        {
          value: 'raw',
          'aria-label': 'Raw view',
          label: (
            <Tooltip title="Raw view">
              <span
                className="flex items-center"
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

// ── FindButton ──────────────────────────────────────────────────────────────
// Toggles the find-in-document bar (rendered by FindableRegion in the body).
// Header + body coordinate through File.store.fileFindOpen. Renders nothing when
// the CSS Custom Highlight API is unavailable — the browser's native find is the
// fallback, so a dead button never appears.

// Platform-aware find shortcut label (⌘F on macOS, Ctrl+F elsewhere). `navigator`
// is always present in the browser; guard for the (test/SSR) absence anyway.
function findShortcutLabel(): string {
  const p =
    typeof navigator !== 'undefined'
      ? navigator.platform || navigator.userAgent
      : ''
  return /mac|iphone|ipad|ipod/i.test(p) ? 'Find (⌘F)' : 'Find (Ctrl+F)'
}

export function FindButton({ file }: { file: FileEntity }) {
  if (!isHighlightSupported()) return null
  const open = Stores.File.fileFindOpen.get(file.id) ?? false
  return (
    <Button
      variant="ghost"
      size="icon"
      tooltip={findShortcutLabel()}
      aria-label="Find in document"
      aria-pressed={open}
      icon={<Search />}
      onClick={() => Stores.File.__state.setFileFindOpen(file.id, !open)}
      data-testid="file-viewer-find-btn"
    />
  )
}

// ── WrapToggle ──────────────────────────────────────────────────────────────
// Toggles word-wrap for the raw/code view. Off (default) keeps long lines on one
// line with horizontal scroll; on wraps them. Coordinated via File.store.fileWordWrap.

export function WrapToggle({ file }: { file: FileEntity }) {
  const on = Stores.File.fileWordWrap.get(file.id) ?? false
  return (
    <Button
      variant={on ? 'default' : 'ghost'}
      size="icon"
      tooltip={on ? 'Word wrap: on' : 'Word wrap: off'}
      aria-label="Toggle word wrap"
      aria-pressed={on}
      icon={<WrapText />}
      onClick={() => Stores.File.__state.setFileWordWrap(file.id, !on)}
      data-testid="file-viewer-wrap-btn"
    />
  )
}

// ── CopySelectionButton ─────────────────────────────────────────────────────
// Copies the currently-selected text when the selection is inside the viewer.
// Distinct from CopyButton (whole-document copy). Warns (not errors) on an empty
// selection so the clipboard is never clobbered.

export function CopySelectionButton() {
  const handleCopy = async () => {
    const selection = window.getSelection()
    const text = selection?.toString() ?? ''
    // Only copy a selection that lies INSIDE a file viewer region — otherwise a
    // stray page selection (sidebar, another panel) would be copied by this
    // viewer's button, which is surprising.
    const anchor = selection?.anchorNode ?? null
    const anchorEl =
      anchor?.nodeType === Node.ELEMENT_NODE
        ? (anchor as Element)
        : anchor?.parentElement ?? null
    // Unquoted attribute value so a quoted testid literal doesn't appear here
    // and trip the global testid-uniqueness guard.
    const inViewer = !!anchorEl?.closest('[data-testid=file-findable-region]')
    if (text.trim() === '' || !inViewer) {
      message.warning('Select text in the document to copy')
      return
    }
    try {
      await navigator.clipboard.writeText(text)
      message.success('Copied selection')
    } catch {
      message.error('Failed to copy')
    }
  }
  return (
    <Button
      variant="ghost"
      size="icon"
      tooltip="Copy selection"
      aria-label="Copy selection"
      icon={<ClipboardCopy />}
      onClick={handleCopy}
      data-testid="file-viewer-copy-selection-btn"
    />
  )
}

// ── OpenInNewTabButton ──────────────────────────────────────────────────────
// Opens the raw file in a new browser tab (browser-native rendering) via a
// freshly-minted download token. Shell-level affordance — file-type agnostic.

export function OpenInNewTabButton({ file }: { file: FileEntity }) {
  return (
    <Button
      variant="ghost"
      size="icon"
      tooltip="Open in new tab"
      aria-label="Open file in new tab"
      icon={<ExternalLink />}
      onClick={() =>
        Stores.File.openFileInNewTab(file.id).catch(() =>
          message.error('Failed to open file'),
        )
      }
      data-testid="file-viewer-open-tab-btn"
    />
  )
}

// ── FullPageButton ──────────────────────────────────────────────────────────
// Navigates to the dedicated full-page in-app view (/files/:id) and closes the
// preview drawer (so returning via Back lands on the originating page, not a
// drawer over it). Shell-level affordance.

export function FullPageButton({ file }: { file: FileEntity }) {
  const navigate = useNavigate()
  return (
    <Button
      variant="ghost"
      size="icon"
      tooltip="Open full page"
      aria-label="Open file full page"
      icon={<Maximize2 />}
      onClick={() => {
        Stores.FilePreviewDrawer.__state.closePreview()
        navigate(`/files/${file.id}`)
      }}
      data-testid="file-viewer-fullpage-btn"
    />
  )
}
