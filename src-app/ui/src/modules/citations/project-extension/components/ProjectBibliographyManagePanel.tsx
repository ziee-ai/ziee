import { useCallback, useEffect, useState } from 'react'
import { ImportOutlined } from '@ant-design/icons'
import { App, Button, Empty, Space, Spin, Typography } from 'antd'
import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/types'
import type { BibliographyEntry } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { CitationCard } from '../../components/CitationCard'
import { ImportCitationsModal } from '../../components/ImportCitationsModal'

const { Text } = Typography

/** Full management of a project's reference list — inside the knowledge drawer. */
export function ProjectBibliographyManagePanel() {
  const { message } = App.useApp()
  // Import-into-project + per-card Delete require manage; gate them so a
  // read-only (`citations::use`) viewer doesn't see actions that would 403.
  const canManage = usePermission(Permissions.CitationsManage)
  const project = Stores.ProjectDetail.project
  const projectId = project?.id ?? null
  const [entries, setEntries] = useState<BibliographyEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [importOpen, setImportOpen] = useState(false)

  const reload = useCallback(async () => {
    if (!projectId) return
    setLoading(true)
    try {
      const r = await ApiClient.Citations.list({ project_id: projectId })
      setEntries(r.entries)
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to load references')
    } finally {
      setLoading(false)
    }
  }, [projectId, message])

  useEffect(() => {
    void reload()
    // Stay current when the library changes elsewhere (import/attach/detach/delete).
    // Group-named subscription (the project's EventBus idiom) auto-dedups.
    const unsub = Stores.EventBus.on(
      'sync:bibliography_entry',
      () => void reload(),
      'ProjectBibliographyManagePanel',
    )
    return () => unsub()
  }, [reload])

  if (!projectId) return <Empty description="Open a project to manage its references." />

  return (
    <Space direction="vertical" style={{ width: '100%' }}>
      <Space>
        {canManage && (
          <Button
            type="primary"
            icon={<ImportOutlined />}
            onClick={() => setImportOpen(true)}
          >
            Import into project
          </Button>
        )}
        <Text type="secondary">{entries.length} reference(s)</Text>
      </Space>

      {loading ? (
        <Spin />
      ) : entries.length === 0 ? (
        <Empty description="No references in this project yet." />
      ) : (
        <div>
          {entries.map(e => (
            <CitationCard key={e.id} entry={e} canManage={canManage} />
          ))}
        </div>
      )}

      <ImportCitationsModal
        open={importOpen}
        projectId={projectId}
        onClose={() => {
          setImportOpen(false)
          void reload()
        }}
      />
    </Space>
  )
}
