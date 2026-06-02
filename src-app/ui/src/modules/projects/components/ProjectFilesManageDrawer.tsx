import { useState } from 'react'
import { Button, Typography } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { ProjectFilesPanel } from '@/modules/projects/components/ProjectFilesPanel'
// FileCard lives in the chat extensions tree but is generic enough to
// reuse here — its only chat-specific behavior (opening the file in
// chat's right panel on click) is overridden by passing `onClick`,
// and the thumbnail lookup against `Stores.Chat.FileStore` safely
// returns `undefined` for project files (falls back to the file-type
// icon). Project module is allowed to import from chat per the
// chat↔project decoupling rule (only chat→project is forbidden).
import { FileCard } from '@/modules/chat/extensions/file/components/FileCard'
import { Stores } from '@/core/stores'

const { Text } = Typography

interface ProjectFilesManageDrawerProps {
  projectId: string
}

/**
 * Inline knowledge-file preview + "Manage files" drawer.
 *
 * Shows EVERY attached file as a FileCard tile on the project detail
 * page — no truncation. The 100-file project cap keeps the worst
 * case bounded (Tailwind flex-wrap handles overflow into multiple
 * rows naturally), and seeing the full set at a glance is what users
 * told us they wanted ("don't make me guess what's already in here").
 * The "Manage" button opens the full ProjectFilesPanel in a side
 * drawer for upload / attach / detach.
 */
export function ProjectFilesManageDrawer({
  projectId,
}: ProjectFilesManageDrawerProps) {
  const [open, setOpen] = useState(false)
  const { files } = Stores.ProjectDetail

  return (
    <>
      {/* Heading row — title left, Manage button pushed to the right
          via ml-auto. The Manage button used to live at the tail of
          the file-tile flex-wrap below, which moved its position
          based on how many files were attached. Pinning it to the
          right of the header makes it discoverable regardless of
          file count + 0-file state. */}
      <div className="flex items-center mb-2">
        <Text strong>Project knowledge</Text>
        <Button
          size="small"
          onClick={() => setOpen(true)}
          aria-label="Manage knowledge files"
          className="!ml-auto"
        >
          Manage
        </Button>
      </div>

      <div className="flex flex-wrap gap-3 items-start">
        {files.length === 0 ? (
          <span
            className="text-sm text-gray-500 italic"
            data-test-files-empty="true"
          >
            No knowledge files yet
          </span>
        ) : (
          files.map(file => (
            <FileCard
              key={file.id}
              file={file}
              variant="square"
              canRemove={false}
              // Clicking a tile opens the manage drawer (where the
              // user can detach / inspect). Overrides FileCard's
              // default chat-right-panel behavior, which isn't
              // applicable on the project detail page.
              onClick={() => setOpen(true)}
            />
          ))
        )}
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
