/**
 * Per-conversation "Mounted folders" control (DESKTOP-ONLY).
 *
 * A header-trailing decoration: a folder button with a count badge that opens
 * a popover to add/remove host folders mounted into the sandbox for THIS
 * conversation (overrides the project's mounts when set). Mounts appear in the
 * sandbox at /mnt/<full host path>, read-only by default.
 */

import { useState } from 'react'
import { App, Badge, Button, Empty, List, Popover, Switch, Typography } from 'antd'
import {
  DeleteOutlined,
  FolderAddOutlined,
  FolderOpenOutlined,
} from '@ant-design/icons'

import type { MountEntry } from '@/api-client/types'
import { Stores } from '@/core/stores'

export function ConversationMountsControl() {
  const { message } = App.useApp()
  const conversationId = Stores.Chat.conversation?.id
  const { saving } = Stores.ConversationHostMounts

  const [open, setOpen] = useState(false)
  const [draft, setDraft] = useState<MountEntry[]>([])

  if (!conversationId) return null

  const savedCount =
    Stores.ConversationHostMounts.byConversation[conversationId]?.length ?? 0

  const onOpenChange = async (next: boolean) => {
    setOpen(next)
    if (next) {
      await Stores.ConversationHostMounts.loadMounts(conversationId)
      setDraft(
        Stores.ConversationHostMounts.__state.byConversation[conversationId] ?? [],
      )
    }
  }

  const addFolder = async () => {
    const picked = await Stores.FileDialog.openFolder({
      title: 'Select a folder to mount into this conversation',
    })
    if (!picked || Array.isArray(picked)) return
    if (draft.some((m) => m.host_path === picked)) {
      message.info('That folder is already mounted')
      return
    }
    setDraft([...draft, { host_path: picked, read_only: true }])
  }

  const save = async () => {
    try {
      await Stores.ConversationHostMounts.saveMounts(conversationId, draft)
      message.success('Saved mounted folders')
      setOpen(false)
    } catch {
      message.error('Failed to save mounted folders')
    }
  }

  const content = (
    <div style={{ width: 360 }} data-test-section="conversation-host-mounts">
      <Typography.Paragraph type="secondary" className="!mb-2 text-xs">
        Folders mounted here apply to this conversation only and override the
        project's. Read-only by default.
      </Typography.Paragraph>
      {draft.length === 0 ? (
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="No folders mounted" />
      ) : (
        <List
          size="small"
          dataSource={draft}
          rowKey={(m) => m.host_path}
          renderItem={(m, i) => (
            <List.Item
              actions={[
                <Switch
                  key="ro"
                  size="small"
                  checked={m.read_only}
                  onChange={(c) =>
                    setDraft(
                      draft.map((x, idx) => (idx === i ? { ...x, read_only: c } : x)),
                    )
                  }
                />,
                <Button
                  key="rm"
                  type="text"
                  danger
                  size="small"
                  icon={<DeleteOutlined />}
                  onClick={() => setDraft(draft.filter((_, idx) => idx !== i))}
                  aria-label={`Remove ${m.host_path}`}
                />,
              ]}
            >
              <Typography.Text code ellipsis style={{ maxWidth: 220 }}>
                {m.host_path}
              </Typography.Text>
            </List.Item>
          )}
        />
      )}
      <div className="mt-2 flex justify-between">
        <Button size="small" icon={<FolderAddOutlined />} onClick={addFolder}>
          Add folder
        </Button>
        <Button size="small" type="primary" onClick={save} loading={saving}>
          Save
        </Button>
      </div>
    </div>
  )

  return (
    <Popover
      open={open}
      onOpenChange={onOpenChange}
      trigger="click"
      placement="bottomRight"
      content={content}
      title="Mounted folders"
    >
      <Badge count={savedCount} size="small" offset={[-2, 2]}>
        <Button
          type="text"
          icon={<FolderOpenOutlined />}
          aria-label="Mounted folders"
        />
      </Badge>
    </Popover>
  )
}
