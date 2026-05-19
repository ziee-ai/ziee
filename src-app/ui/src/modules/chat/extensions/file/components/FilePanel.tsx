import { Typography, theme, Empty } from 'antd'
import { FileUnknownOutlined } from '@ant-design/icons'
import type { File as FileEntity } from '@/api-client/types'
import { getViewer } from '@/modules/chat/extensions/file/fileViewerRegistry'
import { DownloadButton } from '@/modules/chat/extensions/file/file-viewers/shared/chrome'

const { Title, Text } = Typography

interface FilePanelProps {
  file: FileEntity
}

/**
 * Panel shell — owns the title bar and overall layout, delegates everything
 * inside the panel body (and the action area to the right of the title) to
 * the matching viewer's `body` and optional `headerActions` slot components.
 */
export function FilePanel({ file }: FilePanelProps) {
  const { token } = theme.useToken()
  const handler = getViewer(file.filename, file.mime_type ?? undefined)

  // Body + header actions. When no viewer is registered for the file type,
  // we fall through to an explicit "Cannot preview" state below so the
  // panel never silently renders an empty area.
  const Body = handler?.body
  const HeaderActions = handler?.headerActions

  return (
    <div className="flex flex-col h-full w-full" style={{ backgroundColor: token.colorBgLayout }}>
      {/* Title bar — panel-owned. Viewer fills the right-side actions area
          when there's a registered viewer; otherwise we surface Download. */}
      <div
        className="flex items-center gap-2 px-3 py-2 flex-shrink-0"
        style={{ borderBottom: `1px solid ${token.colorBorderSecondary}` }}
      >
        <Title level={5} className="!m-0 flex-1 truncate" title={file.filename}>
          {file.filename}
        </Title>
        {HeaderActions
          ? <HeaderActions file={file} />
          : <DownloadButton file={file} />}
      </div>

      {/* Body — fully owned by the viewer. When no viewer matches, show a
          deliberate "Cannot preview" empty state instead of returning null,
          so the user isn't left staring at a blank panel wondering whether
          the app froze. */}
      <div className="flex-1 overflow-hidden" style={{ backgroundColor: token.colorBgContainer }}>
        {Body
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
