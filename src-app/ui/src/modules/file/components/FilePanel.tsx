import { Typography, theme, Empty } from 'antd'
import { FileUnknownOutlined, WarningOutlined } from '@ant-design/icons'
import type { File as FileEntity } from '@/api-client/types'
import { getViewer } from '@/modules/file/registry/fileViewerRegistry'
import { DownloadButton } from '@/modules/file/viewers/shared/chrome'

const { Title, Text } = Typography

/** Hard cap on previewable file size. Files above this never trigger a
 *  content download — the panel renders a "too large to preview" empty
 *  state instead. The cutoff intentionally covers the common
 *  log/dataset/spreadsheet "kinda big" range (most files <1 MB; large
 *  CSVs / parquets / logs commonly exceed 50 MB). Above 10 MB the cost
 *  of fetching + parsing + rendering (especially shiki highlighting
 *  for code, or xlsx parsing for spreadsheets) starts hurting more
 *  than the preview is worth — Download is still one click away. */
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
}

/**
 * Renders the right-side actions for a file (viewer-specific
 * HeaderActions if the matched viewer declares them, otherwise a
 * Download button). Exported so hosts that render their own title bar
 * (FilePreviewDrawer) can place these actions next to the filename
 * without duplicating the viewer-registry lookup.
 */
export function FilePanelHeaderActions({ file }: { file: FileEntity }) {
  const handler = getViewer(file.filename, file.mime_type ?? undefined)
  const HeaderActions = handler?.headerActions
  return HeaderActions ? <HeaderActions file={file} /> : <DownloadButton file={file} />
}

/**
 * Panel shell — owns the title bar and overall layout, delegates everything
 * inside the panel body (and the action area to the right of the title) to
 * the matching viewer's `body` and optional `headerActions` slot components.
 */
export function FilePanel({ file, hideHeader = false }: FilePanelProps) {
  const { token } = theme.useToken()
  const handler = getViewer(file.filename, file.mime_type ?? undefined)
  const Body = handler?.body
  const tooLarge = file.file_size > PREVIEW_SIZE_LIMIT_BYTES

  return (
    <div className="flex flex-col h-full w-full" style={{ backgroundColor: token.colorBgLayout }}>
      {/* Title bar — panel-owned. Viewer fills the right-side actions area
          when there's a registered viewer; otherwise we surface Download.
          Hosts that render their own header (FilePreviewDrawer) pass
          hideHeader to skip this and avoid duplication. */}
      {!hideHeader && (
        <div
          className="flex items-center gap-2 px-3 py-2 flex-shrink-0"
          style={{ borderBottom: `1px solid ${token.colorBorderSecondary}` }}
        >
          <Title level={5} className="!m-0 flex-1 truncate" title={file.filename}>
            {file.filename}
          </Title>
          {/* Too-large files always get plain Download — viewer-specific
              actions (PDF page nav, CSV controls, etc.) need the body
              loaded to be meaningful. */}
          {tooLarge ? <DownloadButton file={file} /> : <FilePanelHeaderActions file={file} />}
        </div>
      )}

      {/* Body — fully owned by the viewer unless the file exceeds the
          preview-size cap (we skip loading entirely) or no viewer
          matches. Both fallbacks show explicit empty states instead of
          returning null. */}
      <div className="flex-1 overflow-hidden" style={{ backgroundColor: token.colorBgContainer }}>
        {tooLarge
          ? (
            <div
              className="flex flex-col items-center justify-center h-full p-6"
              data-testid="too-large-to-preview"
            >
              <Empty
                image={<WarningOutlined style={{ fontSize: 56, color: token.colorWarning }} />}
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
                image={<FileUnknownOutlined style={{ fontSize: 56, color: token.colorTextQuaternary }} />}
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
