import { useState } from 'react'
import { Button, Tag } from 'antd'
import { FileOutlined } from '@ant-design/icons'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { ProjectFilesPanel } from '@/modules/projects/components/ProjectFilesPanel'
import { Stores } from '@/core/stores'

interface ProjectFilesManageDrawerProps {
  projectId: string
}

/**
 * Compact inline knowledge-file preview + "Manage files" drawer.
 *
 * The project detail page (Option A redesign) shows up to N file
 * chips inline so users see project knowledge at a glance, with a
 * "Manage" button that opens the full ProjectFilesPanel in a side
 * drawer for attach/detach. Keeps conversations the primary content
 * of the page (file management is a secondary concern most users
 * touch infrequently after project setup).
 */
const INLINE_PREVIEW_LIMIT = 5

export function ProjectFilesManageDrawer({
  projectId,
}: ProjectFilesManageDrawerProps) {
  const [open, setOpen] = useState(false)
  const { files } = Stores.ProjectDetail
  const total = files.length
  const preview = files.slice(0, INLINE_PREVIEW_LIMIT)
  const overflow = Math.max(0, total - INLINE_PREVIEW_LIMIT)

  return (
    <>
      <div className="flex flex-wrap gap-2 items-center">
        {preview.length === 0 ? (
          <span
            className="text-sm text-gray-500 italic"
            data-test-files-empty="true"
          >
            No knowledge files yet
          </span>
        ) : (
          preview.map(file => (
            <Tag
              key={file.id}
              icon={<FileOutlined />}
              data-test-file-chip={file.filename}
            >
              {file.filename}
            </Tag>
          ))
        )}
        {overflow > 0 && (
          <Tag
            color="default"
            data-test-files-overflow="true"
          >
            +{overflow} more
          </Tag>
        )}
        <Button
          size="small"
          onClick={() => setOpen(true)}
          aria-label="Manage knowledge files"
        >
          Manage
        </Button>
      </div>

      <Drawer
        title="Project knowledge"
        open={open}
        onClose={() => setOpen(false)}
        size={600}
        destroyOnHidden
        footer={null}
      >
        <ProjectFilesPanel projectId={projectId} />
      </Drawer>
    </>
  )
}
