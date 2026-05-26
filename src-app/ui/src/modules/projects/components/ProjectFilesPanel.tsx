import { App, Button, Empty, List, Popconfirm, Tag, Typography } from 'antd'
import { DeleteOutlined, FileOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type File as ProjectFile } from '@/api-client/types'

interface ProjectFilesPanelProps {
  projectId: string
}

/**
 * 100-file cap per project (server-enforced via PROJECT_MAX_FILES).
 * The UI mirrors the constant here so users see a counter + warning
 * before they hit a 422. Closes audit F2.
 */
const PROJECT_FILE_CAP = 100

export function ProjectFilesPanel({ projectId }: ProjectFilesPanelProps) {
  const { message } = App.useApp()
  const { files, filesLoading } = Stores.ProjectDetail
  // Detach is a project edit, not a file delete — gate by ProjectsEdit
  // so users without edit access can SEE the knowledge files but
  // can't remove them (audit Q5).
  const canEdit = usePermission(Permissions.ProjectsEdit)

  const count = files.length
  const atCap = count >= PROJECT_FILE_CAP
  const nearCap = count >= PROJECT_FILE_CAP - 5 && !atCap

  const handleDetach = async (file: ProjectFile) => {
    try {
      await Stores.ProjectDetail.detachFile(projectId, file.id)
      message.success('File removed from project')
    } catch (err) {
      message.error(
        err instanceof Error ? err.message : 'Failed to detach file',
      )
    }
  }

  const counterChip = (
    <Tag
      color={atCap ? 'error' : nearCap ? 'warning' : 'default'}
      aria-label={`Project file count: ${count} of ${PROJECT_FILE_CAP}`}
    >
      {count} / {PROJECT_FILE_CAP} files
    </Tag>
  )

  if (!filesLoading && files.length === 0) {
    return (
      <div>
        <div className="flex items-center justify-between mb-3">
          <Typography.Text strong>Knowledge files</Typography.Text>
          {counterChip}
        </div>
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description="No knowledge files yet"
        >
          <Typography.Text type="secondary" className="block max-w-md mx-auto">
            Attach files from your library to share their contents with every
            conversation in this project.
          </Typography.Text>
        </Empty>
      </div>
    )
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <Typography.Text strong>Knowledge files</Typography.Text>
        {counterChip}
      </div>
      {atCap && (
        <Typography.Text type="danger" className="block mb-2 text-sm">
          You've reached the {PROJECT_FILE_CAP}-file cap. Remove a file to
          attach a new one.
        </Typography.Text>
      )}
      <List
        loading={filesLoading}
        dataSource={files}
        renderItem={file => (
          <List.Item
            actions={
              canEdit
                ? [
                    <Popconfirm
                      key="detach"
                      title="Remove file from project?"
                      description="The file itself is preserved; only its project attachment is removed."
                      okText="Remove"
                      okButtonProps={{ danger: true }}
                      cancelText="Cancel"
                      onConfirm={() => handleDetach(file)}
                    >
                      <Button
                        type="text"
                        danger
                        icon={<DeleteOutlined />}
                        aria-label={`Remove ${file.filename}`}
                      />
                    </Popconfirm>,
                  ]
                : []
            }
          >
            <List.Item.Meta
              avatar={<FileOutlined />}
              title={file.filename}
              description={file.mime_type ?? 'unknown type'}
            />
          </List.Item>
        )}
      />
    </div>
  )
}
