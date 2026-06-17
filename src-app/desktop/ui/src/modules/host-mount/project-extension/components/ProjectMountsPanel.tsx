// "Mounted folders" panel for the project detail page (desktop-only).
//
// Lists the host folders mounted into the code sandbox for this project, with a
// native folder picker (Stores.FileDialog) to add one, a read-only toggle, and
// remove. Folders appear in the sandbox at /mnt/<full host path> for every chat
// in the project (read-through to conversations).

import { useEffect, useState } from 'react'
import {
  App,
  Button,
  Card,
  Empty,
  List,
  Skeleton,
  Switch,
  Typography,
} from 'antd'
import {
  DeleteOutlined,
  FolderAddOutlined,
  FolderOpenOutlined,
} from '@ant-design/icons'

import type { MountEntry } from '@/api-client/types'
import { Stores } from '@/core/stores'

/** Mirror of the server-side `/mnt/<full host path>` derivation (display only). */
function toSandboxPath(hostPath: string): string {
  let s = hostPath.replace(/\\/g, '/')
  // Strip a Windows drive colon: "C:/x" -> "C/x".
  if (/^[A-Za-z]:/.test(s)) s = s[0] + s.slice(2)
  return '/mnt/' + s.replace(/^\/+/, '')
}

export function ProjectMountsPanel() {
  const { message } = App.useApp()
  const project = Stores.ProjectDetail.project
  const { mounts, loading, saving } = Stores.ProjectHostMounts

  const [draft, setDraft] = useState<MountEntry[]>([])
  useEffect(() => {
    setDraft(mounts)
  }, [mounts])

  if (!project) return null

  const dirty = JSON.stringify(draft) !== JSON.stringify(mounts)

  const addFolder = async () => {
    const picked = await Stores.FileDialog.openFolder({
      title: 'Select a folder to mount into the sandbox',
    })
    if (!picked || Array.isArray(picked)) return
    if (draft.some((m) => m.host_path === picked)) {
      message.info('That folder is already mounted')
      return
    }
    setDraft([...draft, { host_path: picked, read_only: true }])
  }

  const removeAt = (i: number) =>
    setDraft(draft.filter((_, idx) => idx !== i))

  const setReadOnly = (i: number, readOnly: boolean) =>
    setDraft(draft.map((m, idx) => (idx === i ? { ...m, read_only: readOnly } : m)))

  const save = async () => {
    try {
      await Stores.ProjectHostMounts.saveMounts(project.id, draft)
      message.success('Saved mounted folders')
    } catch {
      message.error('Failed to save mounted folders')
    }
  }

  return (
    <Card
      title={
        <span>
          <FolderOpenOutlined className="mr-2" />
          Mounted folders
        </span>
      }
      extra={
        <Button
          type="text"
          icon={<FolderAddOutlined />}
          onClick={addFolder}
          aria-label="Add folder"
        >
          Add folder
        </Button>
      }
      className="mb-4"
      data-test-section="project-host-mounts"
    >
      <Typography.Text type="secondary" className="block mb-4">
        Folders from this machine are mounted into the code sandbox at{' '}
        <Typography.Text code>/mnt/&lt;path&gt;</Typography.Text> for every chat in
        this project — so large data files (BAM/FASTQ/VCF) are read in place, not
        uploaded. Read-only by default.
      </Typography.Text>

      {loading && draft.length === 0 ? (
        <Skeleton active paragraph={{ rows: 2 }} />
      ) : draft.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={
            <Typography.Text type="secondary">No folders mounted</Typography.Text>
          }
        />
      ) : (
        <List
          size="small"
          dataSource={draft}
          rowKey={(m) => m.host_path}
          renderItem={(m, i) => (
            <List.Item
              actions={[
                <span key="ro" className="text-xs">
                  Read-only{' '}
                  <Switch
                    size="small"
                    checked={m.read_only}
                    onChange={(c) => setReadOnly(i, c)}
                  />
                </span>,
                <Button
                  key="rm"
                  type="text"
                  danger
                  icon={<DeleteOutlined />}
                  onClick={() => removeAt(i)}
                  aria-label={`Remove ${m.host_path}`}
                />,
              ]}
            >
              <List.Item.Meta
                title={<Typography.Text code>{m.host_path}</Typography.Text>}
                description={
                  <Typography.Text type="secondary" className="text-xs">
                    → {toSandboxPath(m.host_path)}
                  </Typography.Text>
                }
              />
            </List.Item>
          )}
        />
      )}

      <div className="mt-3 flex justify-end">
        <Button
          type="primary"
          onClick={save}
          loading={saving}
          disabled={!dirty || loading}
        >
          Save
        </Button>
      </div>
    </Card>
  )
}
