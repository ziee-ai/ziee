import { App, Typography, theme } from 'antd'
import { Stores } from '@/core/stores'
import type { File as FileEntity } from '@/api-client/types'
import { FileDrawerBody, DrawerActionButton } from './FilePanelBody'

const { Title } = Typography

interface FilePanelProps {
  file: FileEntity
}

export function FilePanel({ file }: FilePanelProps) {
  const { message } = App.useApp()
  const { token } = theme.useToken()
  const { fileViewModes, setFileViewMode } = Stores.Chat.FileStore
  const viewMode = fileViewModes.get(file.id) ?? 'compiled'

  return (
    <div className="flex flex-col h-full w-full" style={{ backgroundColor: token.colorBgLayout }}>
      {/* Header */}
      <div
        className="flex items-center gap-2 px-3 py-2 flex-shrink-0"
        style={{ borderBottom: `1px solid ${token.colorBorderSecondary}` }}
      >
        <Title level={5} className="!m-0 flex-1 truncate" title={file.filename}>
          {file.filename}
        </Title>
        <DrawerActionButton
          file={file}
          message={message}
          viewMode={viewMode}
          onViewModeChange={mode => setFileViewMode(file.id, mode)}
        />
      </div>

      {/* Body */}
      <div className="flex-1 overflow-hidden" style={{ backgroundColor: token.colorBgContainer }}>
        <FileDrawerBody file={file} viewMode={viewMode} />
      </div>
    </div>
  )
}
