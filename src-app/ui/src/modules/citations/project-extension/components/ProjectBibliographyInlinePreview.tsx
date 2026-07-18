import { useEffect, useState } from 'react'
import { Book } from 'lucide-react'
import { Button, Text } from '@ziee/kit'
import { ApiClient } from '@/api-client'
import { Stores } from '@ziee/framework/stores'
import { useOpenManageDrawer } from '@/modules/projects/core/extensions'

/** Compact view-only summary of a project's reference list (the knowledge card).
 *  Header mirrors the file knowledge card (icon + "References" + count); refetches
 *  on `sync:bibliography_entry` so the count stays current after an import/attach/
 *  detach happens elsewhere. */
export function ProjectBibliographyInlinePreview() {
  const project = Stores.ProjectDetail.project
  const projectId = project?.id
  const openManageDrawer = useOpenManageDrawer()
  // `null` = not loaded / error (distinct from a real 0), so a fetch failure
  // doesn't masquerade as an authoritative "No references".
  const [count, setCount] = useState<number | null>(null)

  useEffect(() => {
    if (!projectId) return
    let cancelled = false
    const reload = () => {
      ApiClient.Citations.list({ project_id: projectId })
        .then(r => {
          if (!cancelled) setCount(r.entries.length)
        })
        .catch(() => {
          if (!cancelled) setCount(null)
        })
    }
    reload()
    const unsub = Stores.EventBus.on(
      'sync:bibliography_entry',
      reload,
      'ProjectBibliographyInlinePreview',
    )
    return () => {
      cancelled = true
      unsub()
    }
  }, [projectId])

  return (
    <div>
      <div className="flex items-center mb-2">
        <Book className="mr-2" />
        <Text strong>References</Text>
        <Text type="secondary" className="ml-2 !text-xs">
          ({count ?? '—'})
        </Text>
      </div>

      {count === 0 ? (
        <Button
          variant="link"
          onClick={openManageDrawer}
          className="!p-0"
          data-testid="cite-bib-inline-manage-link"
        >
          No references yet — click Manage to add.
        </Button>
      ) : (
        <Text type="secondary">
          {count == null ? '—' : `${count} reference(s)`}
        </Text>
      )}
    </div>
  )
}
