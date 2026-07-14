import { useEffect, useState } from 'react'
import { BookOpen } from 'lucide-react'
import { Button, Text } from '@ziee/kit'
import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'
import { useOpenManageDrawer } from '@/modules/projects/core/extensions'

/** Compact view-only summary of a project's attached knowledge bases (the
 *  knowledge card). Mirrors ProjectBibliographyInlinePreview; refetches on
 *  `sync:knowledge_base` so the count stays current after an attach/detach. */
export function ProjectKnowledgeBasesInlinePreview() {
  // Permission gate (layer 3): a user without knowledge_base::use must see NO
  // trace of the KB knowledge kind in a project — not the header, not the count,
  // and no 403-triggering fetch. The knowledge_kinds slot has no permission
  // field, so each kind gates itself.
  const canUse = usePermission(Permissions.KnowledgeBaseUse)
  const project = Stores.ProjectDetail.project
  const projectId = project?.id
  const openManageDrawer = useOpenManageDrawer()
  // `null` = not loaded / error (distinct from a real 0).
  const [count, setCount] = useState<number | null>(null)

  useEffect(() => {
    if (!projectId || !canUse) return
    let cancelled = false
    const reload = () => {
      ApiClient.KnowledgeBase.listProject({ pid: projectId })
        .then(kbs => {
          if (!cancelled) setCount(kbs.length)
        })
        .catch(() => {
          if (!cancelled) setCount(null)
        })
    }
    reload()
    const unsub = Stores.EventBus.on(
      'sync:knowledge_base',
      reload,
      'ProjectKnowledgeBasesInlinePreview',
    )
    return () => {
      cancelled = true
      unsub()
    }
  }, [projectId, canUse])

  // Hide the KB knowledge kind entirely for users lacking knowledge_base::use.
  if (!canUse) return null

  return (
    <div>
      <div className="flex items-center mb-2">
        <BookOpen className="me-2" />
        <Text strong>Knowledge bases</Text>
        <Text type="secondary" className="ms-2 !text-xs">
          ({count ?? '—'})
        </Text>
      </div>

      {count === 0 ? (
        <Button
          variant="link"
          onClick={openManageDrawer}
          className="!p-0"
          data-testid="kb-project-inline-manage-link"
        >
          No knowledge bases yet — click Manage to attach.
        </Button>
      ) : (
        <Text type="secondary">
          {count == null ? '—' : `${count} knowledge base(s)`}
        </Text>
      )}
    </div>
  )
}
