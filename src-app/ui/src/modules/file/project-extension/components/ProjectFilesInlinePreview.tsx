// View-only file tile strip for the project detail page's Knowledge
// card. Replaces the inline preview portion of the old
// `projects/components/ProjectFilesManageDrawer.tsx`. Clicking a tile
// opens the management drawer via the `useOpenManageDrawer` context
// callback supplied by `ProjectKnowledgeSection`.

import { Button, Typography } from 'antd'
import { FileOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { FileCard } from '@/modules/file/components/FileCard'
import { useOpenManageDrawer } from '@/modules/projects/core/extensions'

const { Text } = Typography

export function ProjectFilesInlinePreview() {
  const openManageDrawer = useOpenManageDrawer()
  const { files } = Stores.ProjectFiles
  const project = Stores.ProjectDetail.project

  // Don't render if no project is loaded — the slot host should have
  // already gated on this, but defense-in-depth.
  if (!project) return null

  return (
    <div>
      <div className="flex items-center mb-2">
        <FileOutlined className="mr-2" />
        <Text strong>Knowledge files</Text>
        <Text type="secondary" className="ml-2 !text-xs">
          ({files.length})
        </Text>
      </div>

      <div className="flex flex-wrap gap-3 items-start">
        {files.length === 0 ? (
          <Button
            type="link"
            onClick={openManageDrawer}
            className="!p-0"
            data-test-files-empty="true"
          >
            No knowledge files yet — click Manage to attach.
          </Button>
        ) : (
          files.map(file => (
            <FileCard
              key={file.id}
              file={file}
              variant="square"
              canRemove={false}
              onClick={openManageDrawer}
            />
          ))
        )}
      </div>
    </div>
  )
}
