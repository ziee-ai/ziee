import { useEffect, useRef, useState } from 'react'
import { Button, Tooltip, theme, App } from 'antd'
import {
  RightOutlined,
  DownOutlined,
  ExportOutlined,
  FileOutlined,
  PicRightOutlined,
} from '@ant-design/icons'
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
  if (bytes === undefined) return ''
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
  const { token } = theme.useToken()
  const { message } = App.useApp()
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
  const Icon = viewer?.icon ?? <FileOutlined />
  const label = viewer?.label

  const showBody = canInline && !collapsed && Body !== undefined && inView
  // Render the body via the authenticated `{file}` path when this is a
  // backend-owned artifact; otherwise the URL-based `{source}` path.
  const slotProps: FileViewerSlotProps = file ? { file } : { source }

  const handleOpenInPanel = () => {
    if (!file) return
    Stores.Chat.displayInRightPanel({
      id: file.id,
      title: file.filename,
      type: 'file',
      data: { fileId: file.id },
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
      className="flex flex-col rounded-md overflow-hidden"
      style={{
        border: `1px solid ${token.colorBorderSecondary}`,
        backgroundColor: token.colorBgContainer,
      }}
    >
      {/* Header row */}
      <div
        className="flex items-center gap-2 px-3 py-2"
        style={{
          backgroundColor: token.colorFillTertiary,
          borderBottom: showBody ? `1px solid ${token.colorBorderSecondary}` : 'none',
        }}
      >
        {/* Chevron = ONLY collapse toggle. Only render when the viewer
            actually has an inline body to toggle; otherwise the header is
            the whole UI and a chevron would be a noop. */}
        {canInline && Body && (
          <Button
            type="text"
            size="small"
            aria-label={collapsed ? 'Expand file preview' : 'Collapse file preview'}
            aria-expanded={!collapsed}
            icon={collapsed ? <RightOutlined /> : <DownOutlined />}
            onClick={() => setCollapsed(c => !c)}
            data-testid="inline-file-preview-chevron"
          />
        )}
        <span
          className="flex-shrink-0 inline-flex items-center justify-center"
          style={{ width: 20, height: 20, color: token.colorTextSecondary }}
        >
          {Icon}
        </span>
        <span
          className="font-medium truncate"
          style={{ color: token.colorText }}
          title={displayName}
        >
          {displayName}
        </span>
        <span
          className="text-xs flex-shrink-0"
          style={{ color: token.colorTextTertiary }}
        >
          {label ? <>· {label}</> : null}
          {displaySize !== undefined ? <> · {formatFileSize(displaySize)}</> : null}
        </span>
        <div className="flex-grow" />
        {HeaderActions ? <HeaderActions {...slotProps} /> : null}
        {/* Open in side panel — only for backend-owned files (need a File id
            to drive the panel renderer). */}
        {file ? (
          <Tooltip title="Open in side panel">
            <Button
              type="text"
              size="small"
              icon={<PicRightOutlined />}
              onClick={handleOpenInPanel}
              aria-label="Open file in side panel"
              data-testid="inline-file-preview-open-panel"
            />
          </Tooltip>
        ) : null}
        <Tooltip title="Open in new tab">
          {file ? (
            // File-backed: mint a fresh token via the store action (a plain
            // <a target=_blank> can't carry the bearer header).
            <Button
              type="text"
              size="small"
              icon={<ExportOutlined />}
              onClick={handleOpenInNewTab}
              aria-label="Open file in new tab"
              data-testid="inline-file-preview-open"
            />
          ) : (
            // External MCP link: open the URL directly.
            <Button
              type="text"
              size="small"
              href={source.url}
              target="_blank"
              rel="noopener noreferrer"
              icon={<ExportOutlined />}
              aria-label="Open file in new tab"
              data-testid="inline-file-preview-open"
            />
          )}
        </Tooltip>
      </div>

      {/* Body — viewer's existing component, called with the source variant.
          Body click does NOTHING; only the chevron in the header toggles.

          `inlineFill` viewers (the tabular data grid) measure their own
          container to size a virtual scroll area, so they need a DEFINITE
          height — a bare `max-height` leaves their `height: 100%` resolving to
          `auto`, and their measurement loops toward zero. They get a fixed-
          height box with internal scroll; intrinsic viewers (image / text /
          markdown) keep the content-sized `max-height` box. */}
      {showBody && Body ? (
        <div
          className={
            viewer?.inlineFill
              ? 'overflow-hidden h-[420px]'
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
