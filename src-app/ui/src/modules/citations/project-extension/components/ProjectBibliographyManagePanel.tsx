import { useCallback, useEffect, useState } from 'react'
import { Import } from 'lucide-react'
import { Button, Empty, message, Spin, Tag, Text } from '@ziee/kit'
import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import type { BibliographyEntry } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'
import { CitationCard } from '../../components/CitationCard'
import { ImportCitationsModal } from '../../components/ImportCitationsModal'

/** Full management of a project's reference list — inside the knowledge drawer. */
export function ProjectBibliographyManagePanel() {
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
  }, [projectId])

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

  if (!projectId) return <Empty description="Open a project to manage its references." data-testid="cite-bib-panel-no-project-empty" />

  return (
    <div className="flex flex-col w-full">
      {/* Header mirrors the Knowledge-files panel: title + count chip on the
          left, the primary action on the right. */}
      <div className="flex items-center justify-between gap-2 mb-3 flex-wrap">
        <div className="flex items-center gap-2">
          <Text strong>References</Text>
          <Tag variant="outline" data-testid="cite-bib-panel-count-tag">
            {entries.length} reference{entries.length === 1 ? '' : 's'}
          </Tag>
        </div>
        {canManage && (
          <Button
            variant="default"
            icon={<Import />}
            onClick={() => setImportOpen(true)}
            data-testid="cite-bib-panel-import-button"
          >
            Import
          </Button>
        )}
      </div>

      {loading ? (
        <Spin label="Loading" />
      ) : entries.length === 0 ? (
        <Empty description="No references in this project yet." data-testid="cite-bib-panel-empty" />
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
    </div>
  )
}
