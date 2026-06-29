// "Mounted folders" panel for the project detail page (desktop-only).
//
// Lists the host folders mounted into the code sandbox for this project, with a
// native folder picker (Stores.FileDialog) to add one, a read-only toggle, and
// remove. Folders appear in the sandbox at /mnt/<full host path> for every chat
// in the project (read-through to conversations).

import { useEffect, useState } from 'react'
import {
  Button,
  Card,
  Empty,
  List,
  Skeleton,
  Switch,
  Text,
  message,
} from '@/components/ui'
import { Trash2, FolderPlus, FolderOpen } from 'lucide-react'

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
          <FolderOpen className="mr-2 inline" />
          Mounted folders
        </span>
      }
      extra={
        <Button
          variant="ghost"
          icon={<FolderPlus />}
          onClick={addFolder}
          aria-label="Add folder"
          data-testid="desktop-hostmount-project-add-btn"
        >
          Add folder
        </Button>
      }
      className="mb-4"
      data-test-section="project-host-mounts"
      data-testid="desktop-hostmount-project-card"
    >
      <Text type="secondary" className="block mb-4">
        Folders from this machine are mounted into the code sandbox at{' '}
        <Text code>/mnt/&lt;path&gt;</Text> for every chat in
        this project — so large data files (BAM/FASTQ/VCF) are read in place, not
        uploaded. Read-only by default.
      </Text>

      {loading && draft.length === 0 ? (
        <div className="space-y-2">
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-2/3" />
        </div>
      ) : draft.length === 0 ? (
        <Empty
          description={<Text type="secondary">No folders mounted</Text>}
          data-testid="desktop-hostmount-project-empty"
        />
      ) : (
        <List
          size="sm"
          data-testid="desktop-hostmount-project-list"
          dataSource={draft}
          rowKey={(m) => m.host_path}
          renderItem={(m, i) => (
            <div className="flex items-center justify-between gap-2">
              <div className="min-w-0">
                <Text code>{m.host_path}</Text>
                <Text type="secondary" className="block text-xs">
                  → {toSandboxPath(m.host_path)}
                </Text>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-xs">
                  Read-only{' '}
                  <Switch
                    size="sm"
                    checked={m.read_only}
                    onChange={(c) => setReadOnly(i, c)}
                    data-testid={`desktop-hostmount-project-readonly-${i}`}
                  />
                </span>
                <Button
                  variant="ghost"
                  size="icon"
                  tooltip={`Remove ${m.host_path}`}
                  icon={<Trash2 />}
                  onClick={() => removeAt(i)}
                  aria-label={`Remove ${m.host_path}`}
                  data-testid={`desktop-hostmount-project-remove-${i}`}
                />
              </div>
            </div>
          )}
        />
      )}

      <div className="mt-3 flex justify-end">
        <Button
          onClick={save}
          loading={saving}
          disabled={!dirty || loading}
          data-testid="desktop-hostmount-project-save-btn"
        >
          Save
        </Button>
      </div>
    </Card>
  )
}
