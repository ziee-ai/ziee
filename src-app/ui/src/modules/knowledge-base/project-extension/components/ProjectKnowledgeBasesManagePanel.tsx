import { useCallback, useEffect, useState } from 'react'
import { BookOpen, Plus } from 'lucide-react'
import { Button, Empty, message, Popover, Spin, Tag, Text } from '@ziee/kit'
import { ApiClient } from '@/api-client'
import { Permissions, type KnowledgeBase } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'

/**
 * Full management of a project's attached knowledge bases — inside the knowledge
 * drawer. Mirrors ProjectBibliographyManagePanel: a header with a count chip +
 * an "Attach" picker (of the user's KBs), and a detachable list of attachments.
 * Attaching a KB to a project grounds every conversation in that project.
 */
export function ProjectKnowledgeBasesManagePanel() {
  const canUse = usePermission(Permissions.KnowledgeBaseUse)
  const project = Stores.ProjectDetail.project
  const projectId = project?.id ?? null
  // The user's full KB library (for the attach picker) comes from the store.
  const { items: allKbs } = Stores.KnowledgeBases
  const [attached, setAttached] = useState<KnowledgeBase[]>([])
  const [loading, setLoading] = useState(false)
  const [busyId, setBusyId] = useState<string | null>(null)

  const reload = useCallback(async () => {
    if (!projectId || !canUse) return
    setLoading(true)
    try {
      setAttached(await ApiClient.KnowledgeBase.listProject({ pid: projectId }))
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to load knowledge bases')
    } finally {
      setLoading(false)
    }
  }, [projectId, canUse])

  useEffect(() => {
    void reload()
    const unsub = Stores.EventBus.on(
      'sync:knowledge_base',
      () => void reload(),
      'ProjectKnowledgeBasesManagePanel',
    )
    return () => unsub()
  }, [reload])

  const attachedIds = new Set(attached.map(k => k.id))
  const attachable = Array.from(allKbs.values()).filter(k => !attachedIds.has(k.id))

  const attach = async (kbId: string) => {
    if (!projectId) return
    setBusyId(kbId)
    try {
      await ApiClient.KnowledgeBase.attachProject({ pid: projectId, kb_id: kbId })
      await reload()
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to attach')
    } finally {
      setBusyId(null)
    }
  }
  const detach = async (kbId: string) => {
    if (!projectId) return
    setBusyId(kbId)
    try {
      await ApiClient.KnowledgeBase.detachProject({ pid: projectId, kb_id: kbId })
      await reload()
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to detach')
    } finally {
      setBusyId(null)
    }
  }

  // Permission gate (layer 3): hide the whole KB manage panel — header, count,
  // list, and picker — for users lacking knowledge_base::use. The `canUse`
  // guards on the buttons below stay as defense-in-depth for the use-vs-manage
  // split, but the panel must not render (or fetch) at all without `use`.
  if (!canUse) return null

  if (!projectId)
    return (
      <Empty
        description="Open a project to manage its knowledge bases."
        data-testid="kb-project-panel-no-project-empty"
      />
    )

  const picker = (
    <div data-testid="kb-project-attach-options" style={{ minWidth: 220, margin: -4 }}>
      {attachable.length === 0 ? (
        <div className="px-3 py-1.5 text-sm text-muted-foreground">
          No more knowledge bases to attach.
        </div>
      ) : (
        attachable.map(kb => (
          <div
            key={kb.id}
            data-testid={`kb-project-attach-option-${kb.id}`}
            role="button"
            tabIndex={0}
            onClick={() => void attach(kb.id)}
            onKeyDown={e => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault()
                void attach(kb.id)
              }
            }}
            className="flex cursor-pointer items-center gap-2 rounded-md px-3 py-1.5 text-sm text-foreground hover:bg-muted focus-visible:outline focus-visible:outline-2"
          >
            <Plus className="size-4 shrink-0" />
            <span className="min-w-0 flex-1 truncate">{kb.name}</span>
            <span className="shrink-0 text-xs text-muted-foreground">
              {kb.document_count}
            </span>
          </div>
        ))
      )}
    </div>
  )

  return (
    <div className="flex flex-col w-full">
      <div className="flex items-center justify-between gap-2 mb-3 flex-wrap">
        <div className="flex items-center gap-2">
          <Text strong>Knowledge bases</Text>
          <Tag variant="outline" data-testid="kb-project-panel-count-tag">
            {attached.length} knowledge base{attached.length === 1 ? '' : 's'}
          </Tag>
        </div>
        {canUse && (
          <Popover content={picker} side="bottom" align="end" className="w-auto">
            <Button variant="default" icon={<Plus />} data-testid="kb-project-attach-button">
              Attach
            </Button>
          </Popover>
        )}
      </div>

      {loading && attached.length === 0 ? (
        <div className="flex justify-center py-6">
          <Spin label="Loading knowledge bases" />
        </div>
      ) : attached.length === 0 ? (
        <Empty
          description="No knowledge bases attached. Use Attach to ground this project's conversations."
          data-testid="kb-project-panel-empty"
        />
      ) : (
        <ul className="space-y-2" data-testid="kb-project-panel-list">
          {attached.map(kb => (
            <li
              key={kb.id}
              className="flex items-center gap-2 rounded-md border border-border p-2"
              data-testid={`kb-project-row-${kb.id}`}
            >
              <BookOpen className="size-4 shrink-0 text-muted-foreground" />
              <span className="min-w-0 flex-1 truncate">{kb.name}</span>
              <Tag
                variant="outline"
                tone="info"
                className="m-0"
                data-testid={`kb-project-row-index-${kb.id}`}
              >
                {kb.indexing_summary.indexed}/{kb.indexing_summary.total} indexed
              </Tag>
              {canUse && (
                <Button
                  variant="link"
                  size="default"
                  loading={busyId === kb.id}
                  onClick={() => void detach(kb.id)}
                  data-testid={`kb-project-detach-${kb.id}`}
                >
                  Detach
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
