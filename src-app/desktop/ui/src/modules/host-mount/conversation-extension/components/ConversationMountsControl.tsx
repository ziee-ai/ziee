/**
 * Per-conversation "Mounted folders" control (DESKTOP-ONLY).
 *
 * A header-trailing decoration: a folder button with a count badge that opens
 * a popover to add/remove host folders mounted into the sandbox for THIS
 * conversation (overrides the project's mounts when set). Mounts appear in the
 * sandbox at /mnt/<full host path>, read-only by default.
 */

import { useState } from 'react'
import { Badge, Button, Empty, List, Paragraph, Popover, Switch, Text, message } from '@/components/ui'
import { Trash2, FolderPlus, FolderOpen } from 'lucide-react'

import type { MountEntry } from '@/api-client/types'
import { Stores } from '@/core/stores'

export function ConversationMountsControl() {
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
    <div className="w-[360px]" data-test-section="conversation-host-mounts">
      <Paragraph type="secondary" className="!mb-2 text-xs">
        Folders mounted here apply to this conversation only and override the
        project's. Read-only by default.
      </Paragraph>
      {draft.length === 0 ? (
        <Empty description="No folders mounted" data-testid="desktop-hostmount-conv-empty" />
      ) : (
        <List
          size="sm"
          data-testid="desktop-hostmount-conv-list"
          aria-label="Mounted folders"
          dataSource={draft}
          rowKey={(m) => m.host_path}
          renderItem={(m, i) => (
            <div className="flex items-center justify-between gap-2">
              <Text code ellipsis className="max-w-[220px]">
                {m.host_path}
              </Text>
              <div className="flex items-center gap-1">
                <Switch
                  size="sm"
                  checked={m.read_only}
                  data-testid={`desktop-hostmount-conv-readonly-${i}`}
                  aria-label={`Read-only ${m.host_path}`}
                  onChange={(c) =>
                    setDraft(
                      draft.map((x, idx) => (idx === i ? { ...x, read_only: c } : x)),
                    )
                  }
                />
                <Button
                  variant="ghost"
                  size="default"
                  icon={<Trash2 />}
                  onClick={() => setDraft(draft.filter((_, idx) => idx !== i))}
                  data-testid={`desktop-hostmount-conv-remove-${i}`}
                  aria-label={`Remove ${m.host_path}`}
                />
              </div>
            </div>
          )}
        />
      )}
      <div className="mt-2 flex justify-between">
        <Button size="default" variant="outline" icon={<FolderPlus />} onClick={addFolder} data-testid="desktop-hostmount-conv-add-btn">
          Add folder
        </Button>
        <Button size="default" onClick={save} loading={saving} data-testid="desktop-hostmount-conv-save-btn">
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
      side="bottom"
      align="end"
      content={content}
      title="Mounted folders"
    >
      <Badge count={savedCount} offset={[-2, 2]} aria-label={`${savedCount} mounted folders`} data-testid="desktop-hostmount-conv-badge">
        <Button
          variant="ghost"
          icon={<FolderOpen />}
          data-testid="desktop-hostmount-conv-trigger-btn"
          aria-label="Mounted folders"
        />
      </Badge>
    </Popover>
  )
}
