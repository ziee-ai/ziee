// View-only file tile strip for the project detail page's Knowledge
// card. Clicking a tile opens the global file-preview drawer
// (FileCard's default click handler). The Manage button on the
// section header (and the empty-state link below) opens the knowledge
// management drawer for file CRUD.

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
        // CSS Grid with `auto-fill` + `minmax(<min>, 1fr)`: each row
        // packs as many tracks as fit at >= the min width, and `1fr`
        // makes them share leftover space equally so the row is
        // always filled exactly (no trailing whitespace like
        // flex-wrap leaves). Cards stay square via FileCard's
        // aspect-ratio enforcer when `stretch` is on, so as the
        // container resizes the cards grow/shrink in lockstep until
        // a new column fits and the layout re-flows.
        //
        // The min (100px) is the "trigger" — when stretching a row
        // would push cards below 100 px, the grid drops one column;
        // when stretching would push them above ~2× the min (i.e.
        // there's room for another column at the minimum width), a
        // new column is added. Tune up for fewer-larger, down for
        // more-smaller. 100 px ≈ the chat composer's 96 px fixed
        // size so cards stay visually compact on wide containers
        // while still expanding to fill each row.
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
