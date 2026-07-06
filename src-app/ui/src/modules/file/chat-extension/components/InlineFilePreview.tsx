import { ChevronRight, ChevronDown, FileOutput, File, PanelRight } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Button, Tooltip, message } from '@/components/ui'
import { Stores } from '@/core/stores'
import type { File as FileEntity } from '@/api-client/types'
import type { FileViewerEntry, FileViewerSlotProps, InlineFileSource } from '@/modules/file/types/viewer'
import { isInlineCapable } from '@/modules/file/viewers/shared/source'

interface InlineFilePreviewProps {
  /** Viewer matched by `getViewer(name, mimeType)`. `undefined` when no
   *  viewer claims this MIME/ext — falls back to a header-only file card. */
  viewer: FileViewerEntry | undefined
  source: InlineFileSource
  /** Resolved File entity when this link is a backend-owned artifact. When
   *  present, the body renders through the authenticated `{file}` path (same
   *  as the right-side panel) and the header gains an "Open in side panel"
   *  button. Absent for external MCP links (URL-based `{source}` path). */
  file?: FileEntity
}

function formatFileSize(bytes: number | undefined): string {
  // A missing / malformed size (undefined, null, NaN) must never render as
  // "NaN GB" — show nothing instead.
  if (bytes === undefined || !Number.isFinite(bytes)) return ''
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

/**
 * Collapsible wrapper around a single tool-result file.
 *
 * - **Header** (always visible): viewer icon + filename + label + size +
 *   the viewer's `headerActions` + "Open in new tab" link + chevron.
 * - **Body** (when expanded AND viewer is inline-capable for this MIME):
 *   the viewer's `body` rendered with the `{source}` variant of
 *   `FileViewerSlotProps`. Otherwise no body — the header link is the
 *   entire UI.
 *
 * The chevron is the ONLY collapse toggle — clicking the body itself
 * does nothing. This matches the right-panel UX where the body is the
 * content, not a button.
 */
export function InlineFilePreview({ viewer, source, file }: InlineFilePreviewProps) {
  const [collapsed, setCollapsed] = useState(false)

  // Viewport-gate the body: a conversation can hold many inline files, and the
  // body is where the cost lives (image thumbnail fetch+decode, text fetch).
  // Mount the body only once this preview scrolls within ~800px of the
  // viewport, then keep it mounted (mount-once: scrolling away does not unmount
  // / refetch). The header is always cheap and always rendered. Combined with
  // the conversation page's instant initial scroll, off-screen files on reload
  // never enter the viewport, so they never fetch.
  //
  // Intentional: a file produced mid-stream while the user has scrolled up
  // shows its header immediately but defers its body until they scroll back
  // within range — same lazy contract as any other off-screen preview. We do
  // NOT special-case the streaming turn here, since that would require the
  // file module to read chat-streaming state (cross-module coupling).
  const containerRef = useRef<HTMLDivElement>(null)
  const [inView, setInView] = useState(false)
  useEffect(() => {
    if (inView) return
    if (typeof IntersectionObserver === 'undefined') {
      setInView(true)
      return
    }
    const el = containerRef.current
    if (!el) return
    const observer = new IntersectionObserver(
      entries => {
        if (entries.some(e => e.isIntersecting)) {
          setInView(true)
          observer.disconnect()
        }
      },
      { rootMargin: '800px 0px' },
    )
    observer.observe(el)
    return () => observer.disconnect()
  }, [inView])

  // Prefer the resolved File's metadata (authoritative) over the link's.
  const displayName = file?.filename ?? source.name
  const displayMime = file?.mime_type ?? source.mimeType
  const displaySize = file?.file_size ?? source.size

  const canInline = isInlineCapable(viewer, displayName, displayMime ?? undefined)
  const Body = canInline ? viewer?.body : undefined
  // Only show the viewer's headerActions when the body itself renders
  // inline. Non-inline viewers (pdf / web / unknown) don't get header
  // chrome here — their existing headers would just return null otherwise.
  const HeaderActions = canInline ? viewer?.headerActions : undefined
  const Icon = viewer?.icon ?? <File />
  const label = viewer?.label

  const showBody = canInline && !collapsed && Body !== undefined && inView
  // Render the body via the authenticated `{file}` path when this is a
  // backend-owned artifact; otherwise the URL-based `{source}` path.
  const slotProps: FileViewerSlotProps = file ? { file } : { source }

  const handleOpenInPanel = () => {
    if (!file) return
    // `__state` (not the render-only proxy) — the proxy fires React hooks on
    // access, a Rules-of-Hooks violation inside an event handler.
    // Pin the panel to the version this resource_link referenced (head if the
    // link carried no version) so a historical tool result opens its own bytes.
    Stores.Chat.__state.displayInRightPanel({
      id: file.id,
      title: file.filename,
      type: 'file',
      data: { fileId: file.id, version: source.version },
    })
  }

  const handleOpenInNewTab = () => {
    if (!file) return
    Stores.File.openFileInNewTab(file.id).catch(() =>
      message.error('Failed to open file'),
    )
  }

  return (
    <div
      ref={containerRef}
      data-testid="inline-file-preview"
      data-file-uri={source.url}
      data-file-id={file?.id}
      className="flex flex-col rounded-md overflow-hidden border border-border bg-card"
    >
      {/* Header row */}
      <div
        className="flex items-center gap-2 px-3 py-2 bg-muted/60"
        style={{
          borderBottom: showBody ? '1px solid var(--border)' : 'none',
        }}
      >
        {/* Chevron = ONLY collapse toggle. Only render when the viewer
            actually has an inline body to toggle; otherwise the header is
            the whole UI and a chevron would be a noop. */}
        {canInline && Body && (
          <Button
            variant="ghost"
            size="default"
            aria-label={collapsed ? 'Expand file preview' : 'Collapse file preview'}
            aria-expanded={!collapsed}
            icon={collapsed ? <ChevronRight /> : <ChevronDown />}
            onClick={() => setCollapsed(c => !c)}
            data-testid="inline-file-preview-chevron"
          />
        )}
        <span
          className="flex-shrink-0 inline-flex items-center justify-center text-muted-foreground"
          style={{ width: 20, height: 20 }}
          data-testid="inline-file-preview-icon"
        >
          {Icon}
        </span>
        <span
          className="font-medium truncate text-foreground"
          title={displayName}
        >
          {displayName}
        </span>
        <span className="text-xs flex-shrink-0 text-muted-foreground">
          {label ? <>· {label}</> : null}
          {displaySize !== undefined ? <> · {formatFileSize(displaySize)}</> : null}
        </span>
        <div className="flex-grow" />
        {HeaderActions ? <HeaderActions {...slotProps} /> : null}
        {/* Open in side panel — only for backend-owned files (need a File id
            to drive the panel renderer). */}
        {file ? (
          <Tooltip content="Open in side panel">
            <Button
              variant="ghost"
              size="default"
              icon={<PanelRight />}
              onClick={handleOpenInPanel}
              aria-label="Open file in side panel"
              data-testid="inline-file-preview-open-panel"
            />
          </Tooltip>
        ) : null}
        <Tooltip content="Open in new tab">
          {file ? (
            // File-backed: mint a fresh token via the store action (a plain
            // <a target=_blank> can't carry the bearer header).
            <Button
              variant="ghost"
              size="default"
              icon={<FileOutput />}
              onClick={handleOpenInNewTab}
              aria-label="Open file in new tab"
              data-testid="inline-file-preview-open"
            />
          ) : (
            // External MCP link: open the URL directly.
            <Button
              variant="ghost"
              size="default"
              href={source.url}
              target="_blank"
              rel="noopener noreferrer"
              icon={<FileOutput />}
              aria-label="Open file in new tab"
              data-testid="inline-file-preview-open"
            />
          )}
        </Tooltip>
      </div>

      {/* Body — viewer's existing component, called with the source variant.
          Body click does NOTHING; only the chevron in the header toggles.

          Both branches are content-sized boxes with a max cap, so a SHORT table
          (or any short body) shrinks to fit instead of leaving empty space. The
          `inlineFill` viewers (the tabular data grid) own their own
          OverlayScrollbars scroll region (`max-h-full` inside), which caps +
          scrolls a tall grid within this box — no fixed height needed (the kit
          Table isn't virtualized, so the old "needs a definite height for
          measurement" constraint no longer applies). */}
      {showBody && Body ? (
        <div
          className={
            viewer?.inlineFill
              ? // inlineFill viewers (the tabular data grid) get a definite,
                // bounded height + scroll. Small grids render as a plain table
                // (all rows present, scrolls within this box); large grids
                // switch to row virtualization, which needs a measurable
                // viewport height — a bare content-sized box collapses to 0 and
                // renders no data rows.
                'overflow-auto h-[min(360px,55vh)]'
              : 'overflow-auto max-h-[600px]'
          }
          data-testid="inline-file-preview-body"
        >
          <Body {...slotProps} />
        </div>
      ) : null}
    </div>
  )
}
