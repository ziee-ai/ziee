import { useState } from 'react'
import { Button, Tooltip, theme } from 'antd'
import {
  RightOutlined,
  DownOutlined,
  ExportOutlined,
  FileOutlined,
} from '@ant-design/icons'
import type { FileViewerEntry, InlineFileSource } from '../types'
import { isInlineCapable } from '../file-viewers/shared/source'

interface InlineFilePreviewProps {
  /** Viewer matched by `getViewer(name, mimeType)`. `undefined` when no
   *  viewer claims this MIME/ext — falls back to a header-only file card. */
  viewer: FileViewerEntry | undefined
  source: InlineFileSource
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
export function InlineFilePreview({ viewer, source }: InlineFilePreviewProps) {
  const { token } = theme.useToken()
  const [collapsed, setCollapsed] = useState(false)

  const canInline = isInlineCapable(viewer, source.name, source.mimeType)
  const Body = canInline ? viewer?.body : undefined
  // Only show the viewer's headerActions when the body itself renders
  // inline. Non-inline viewers (pdf / web / unknown) don't get header
  // chrome here — their existing headers are FileStore-coupled and
  // would just return null for `{source}` anyway. Saves wasted renders
  // and keeps the inline header lean (filename + open-in-new-tab).
  const HeaderActions = canInline ? viewer?.headerActions : undefined
  const Icon = viewer?.icon ?? <FileOutlined />
  const label = viewer?.label

  const showBody = canInline && !collapsed && Body !== undefined
  const slotProps = { source } as const

  return (
    <div
      data-testid="inline-file-preview"
      data-file-uri={source.url}
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
          title={source.name}
        >
          {source.name}
        </span>
        <span
          className="text-xs flex-shrink-0"
          style={{ color: token.colorTextTertiary }}
        >
          {label ? <>· {label}</> : null}
          {source.size !== undefined ? <> · {formatFileSize(source.size)}</> : null}
        </span>
        <div className="flex-grow" />
        {HeaderActions ? <HeaderActions {...slotProps} /> : null}
        <Tooltip title="Open in new tab">
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
        </Tooltip>
      </div>

      {/* Body — viewer's existing component, called with the source variant.
          Body click does NOTHING; only the chevron in the header toggles. */}
      {showBody && Body ? (
        <div
          className="overflow-auto max-h-[600px]"
          data-testid="inline-file-preview-body"
        >
          <Body {...slotProps} />
        </div>
      ) : null}
    </div>
  )
}
