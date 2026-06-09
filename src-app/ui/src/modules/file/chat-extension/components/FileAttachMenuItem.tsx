import { App, Upload, theme } from 'antd'
import { PaperClipOutlined } from '@ant-design/icons'
import type { UploadProps } from 'antd'
import { Stores } from '@/core/stores'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'

const MAX_FILE_SIZE = 100 * 1024 * 1024

/**
 * FileAttachMenuItem Component
 * Menu item inside the + dropdown for attaching files
 */
export function FileAttachMenuItem() {
  const { message } = App.useApp()
  const { token } = theme.useToken()
  const { uploadFiles } = Stores.File
  const { close } = usePlusDropdown()

  const handleBeforeUpload: UploadProps['beforeUpload'] = (file, fileList) => {
    if (file.size > MAX_FILE_SIZE) {
      message.error(`File ${file.name} is too large. Maximum size is 100MB.`)
      return Upload.LIST_IGNORE
    }

    const isLastFile = fileList[fileList.length - 1] === file
    if (isLastFile) {
      const files = fileList.filter(f => f.size <= MAX_FILE_SIZE)
      if (files.length > 0) {
        uploadFiles(files as File[]).catch((error: any) => {
          console.error('Upload failed:', error)
          message.error('Failed to upload files')
        })
      }
    }

    return false
  }

  return (
    <Upload multiple showUploadList={false} beforeUpload={handleBeforeUpload} accept="*/*">
      <div
        className="flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer"
        style={{ color: token.colorTextBase, minWidth: 180 }}
        onClick={close}
        onMouseEnter={e => {
          e.currentTarget.style.backgroundColor = token.colorFillSecondary
        }}
        onMouseLeave={e => {
          e.currentTarget.style.backgroundColor = 'transparent'
        }}
      >
        <PaperClipOutlined style={{ fontSize: 16 }} />
        <span style={{ fontSize: 14 }}>Attach files or photos</span>
      </div>
    </Upload>
  )
}
