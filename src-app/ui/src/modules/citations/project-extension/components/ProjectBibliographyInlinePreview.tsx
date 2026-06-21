import { useEffect, useState } from 'react'
import { Typography } from 'antd'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'

const { Text } = Typography

/** Compact view-only summary of a project's reference list (the knowledge card).
 *  Refetches on `sync:bibliography_entry` so the count stays current after an
 *  import/attach/detach happens elsewhere (mirrors the sync-wired file panel). */
export function ProjectBibliographyInlinePreview() {
  const project = Stores.ProjectDetail.project
  const projectId = project?.id
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
    <Text type="secondary">
      {count == null ? '—' : `${count} reference(s)`}
    </Text>
  )
}
