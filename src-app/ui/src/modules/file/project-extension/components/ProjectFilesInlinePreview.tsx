import { File } from 'lucide-react'
import { Button, Spin } from '@ziee/kit'
import { Text } from '@ziee/kit'
import { Stores } from '@/core/stores'
import { FileCard } from '@/modules/file/components/FileCard'
import { useOpenManageDrawer } from '@/modules/projects/core/extensions'

export function ProjectFilesInlinePreview() {
  const openManageDrawer = useOpenManageDrawer()
  const { files, filesLoading } = Stores.ProjectFiles
  const project = Stores.ProjectDetail.project

  if (!project) return null

  return (
    <div>
      <div className="flex items-center mb-2">
        <File className="mr-2" />
        <Text strong>Knowledge files</Text>
        <Text type="secondary" className="ml-2 !text-xs">
          ({files.length})
        </Text>
      </div>

      {filesLoading && files.length === 0 ? (
        <div className="flex justify-center py-4">
          <Spin size="sm" label="Loading files" />
        </div>
      ) : files.length === 0 ? (
        <Button
          variant="link"
          onClick={openManageDrawer}
          className="!p-0"
          data-test-files-empty="true"
          data-testid="file-project-inline-manage-link"
        >
          No knowledge files yet — click Manage to attach.
        </Button>
      ) : (
        <div
          className="grid gap-3 items-start"
          style={{
            gridTemplateColumns: 'repeat(auto-fill, minmax(100px, 1fr))',
          }}
        >
          {files.map(file => (
            <FileCard
              key={file.id}
              file={file}
              variant="square"
              stretch
              canRemove={false}
            />
          ))}
        </div>
      )}
    </div>
  )
}
