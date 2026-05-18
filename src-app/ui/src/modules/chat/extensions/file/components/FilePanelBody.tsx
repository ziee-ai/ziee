import { Button, Dropdown, Space } from 'antd'
import {
  DownOutlined,
  DownloadOutlined,
  EyeOutlined,
  CodeOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { File as FileEntity } from '@/api-client/types'
import type { MessageInstance } from 'antd/es/message/interface'
import { getViewer } from '../fileViewerRegistry'
import { RawCodeView } from '../file-viewers/shared/RawCodeView'

// ─── Drawer body ─────────────────────────────────────────────────────────────

export function FileDrawerBody({
  file,
  viewMode,
}: {
  file: FileEntity
  viewMode: 'compiled' | 'raw'
}) {
  const handler = getViewer(file.filename, file.mime_type ?? undefined)
  if (!handler) return null
  const { fileTextContents } = Stores.Chat.FileStore
  if (handler.compilable && viewMode === 'raw') {
    const content = fileTextContents.get(file.id) ?? ''
    return <RawCodeView text={content} />
  }
  const Renderer = handler.render
  return <Renderer file={file} />
}

// ─── Panel header actions ─────────────────────────────────────────────────────

export function DrawerActionButton({
  file,
  message,
  viewMode,
  onViewModeChange,
}: {
  file: FileEntity
  message: MessageInstance
  viewMode: 'compiled' | 'raw'
  onViewModeChange: (mode: 'compiled' | 'raw') => void
}) {
  const handler = getViewer(file.filename, file.mime_type ?? undefined)
  const compilable = handler?.compilable ?? false
  const canCopy = handler?.canCopy ?? false

  return (
    <Space>
      {compilable && (
        <Space.Compact>
          <Button
            icon={<EyeOutlined />}
            type={viewMode === 'compiled' ? 'primary' : 'default'}
            title="Compiled view"
            onClick={() => onViewModeChange('compiled')}
          />
          <Button
            icon={<CodeOutlined />}
            type={viewMode === 'raw' ? 'primary' : 'default'}
            title="Raw view"
            onClick={() => onViewModeChange('raw')}
          />
        </Space.Compact>
      )}
      {canCopy && (
        <Space.Compact>
          <Button
            style={{ fontSize: 15 }}
            onClick={async () => {
              try {
                const text = Stores.Chat.FileStore.fileTextContents.get(file.id) ?? ''
                await navigator.clipboard.writeText(text)
                message.success('Copied to clipboard')
              } catch {
                message.error('Failed to copy')
              }
            }}
          >
            Copy
          </Button>
          <Dropdown
            menu={{
              items: [
                {
                  key: 'download',
                  label: 'Download',
                  style: { fontSize: 15 },
                  onClick: () => {
                    Stores.Chat.FileStore.downloadFile(file)
                      .catch(() => message.error('Failed to download file'))
                  },
                },
              ],
              style: { padding: 8 },
            }}
            trigger={['click']}
          >
            <Button icon={<DownOutlined style={{ fontSize: 11 }} />} />
          </Dropdown>
        </Space.Compact>
      )}
      {!canCopy && (
        <Button
          icon={<DownloadOutlined />}
          onClick={() => {
            Stores.Chat.FileStore.downloadFile(file)
              .catch(() => message.error('Failed to download file'))
          }}
        >
          Download
        </Button>
      )}
    </Space>
  )
}
