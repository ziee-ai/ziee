import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Input, Popover, message } from '@ziee/kit'
import { BookOpen, Check, ChevronRight } from 'lucide-react'
import { type KnowledgeBase, Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'

/** Compact per-KB status suffix for a picker row (from indexing_summary). */
function statusSuffix(kb: KnowledgeBase): { text: string; className: string } | null {
  const s = kb.indexing_summary
  if (s.failed > 0) return { text: `${s.failed} failed`, className: 'text-destructive' }
  if (s.indexing + s.pending > 0)
    return { text: `${s.indexing + s.pending} indexing`, className: 'text-muted-foreground' }
  if (s.total === 0) return { text: 'empty', className: 'text-muted-foreground' }
  return null
}

/**
 * KbMenuItem — the "+" dropdown row for grounding the conversation on knowledge
 * bases. Opens a submenu listing the user's KBs; each row TOGGLES attach/detach.
 * Searchable (when there are many), shows per-KB index status, and — when the
 * user has no KBs — links to /knowledge instead of hiding.
 */
export function KbMenuItem() {
  const navigate = useNavigate()
  const canUse = usePermission(Permissions.KnowledgeBaseUse)
  const { items } = Stores.KnowledgeBases
  const { selectedKbIds } = Stores.KnowledgeBaseComposer
  const [query, setQuery] = useState('')

  if (!canUse) return null

  const kbs = Array.from(items.values())
  const filtered = query.trim()
    ? kbs.filter(k => k.name.toLowerCase().includes(query.trim().toLowerCase()))
    : kbs

  const toggle = (id: string) => {
    const p = selectedKbIds.has(id)
      ? Stores.KnowledgeBaseComposer.detach(id)
      : Stores.KnowledgeBaseComposer.attach(id)
    p.catch((e: unknown) =>
      message.error(e instanceof Error ? e.message : 'Failed to update knowledge bases'),
    )
  }

  const popoverContent = (
    <div data-testid="kb-menu-options" style={{ minWidth: 220, margin: -4 }}>
      {kbs.length === 0 ? (
        // Empty → link to management, instead of a dead end.
        <div
          data-testid="kb-menu-empty"
          role="button"
          tabIndex={0}
          onClick={() => navigate('/knowledge')}
          onKeyDown={e => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault()
              navigate('/knowledge')
            }
          }}
          className="cursor-pointer rounded-md px-3 py-2 text-sm text-muted-foreground hover:bg-muted focus-visible:outline focus-visible:outline-2"
        >
          No knowledge bases yet — create one →
        </div>
      ) : (
        <>
          {kbs.length > 6 && (
            <div className="px-2 pb-2 pt-1">
              <Input
                data-testid="kb-menu-search"
                value={query}
                onChange={e => setQuery(e.target.value)}
                placeholder="Filter knowledge bases…"
                onClick={e => e.stopPropagation()}
              />
            </div>
          )}
          {filtered.map(kb => {
            const active = selectedKbIds.has(kb.id)
            const status = statusSuffix(kb)
            return (
              <div
                key={kb.id}
                data-testid={`kb-option-${kb.id}`}
                role="button"
                tabIndex={0}
                aria-pressed={active}
                onClick={() => toggle(kb.id)}
                onKeyDown={e => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    toggle(kb.id)
                  }
                }}
                className={`flex cursor-pointer items-center gap-2 rounded-md px-3 py-1.5 text-sm hover:bg-muted focus-visible:outline focus-visible:outline-2 ${
                  active ? 'text-primary' : 'text-foreground'
                }`}
              >
                <Check className={`size-4 shrink-0 ${active ? 'opacity-100' : 'opacity-0'}`} />
                <span className="min-w-0 flex-1 truncate">{kb.name}</span>
                {status && (
                  <span className={`shrink-0 text-xs ${status.className}`}>{status.text}</span>
                )}
                <span className="shrink-0 text-xs text-muted-foreground">{kb.document_count}</span>
              </div>
            )
          })}
          {filtered.length === 0 && (
            <div className="px-3 py-2 text-sm text-muted-foreground">No matches.</div>
          )}
        </>
      )}
    </div>
  )

  return (
    <Popover content={popoverContent} side="right" align="start" className="w-auto">
      <div
        data-testid="kb-menu-trigger"
        className="flex items-center gap-2 rounded-md px-3 py-1.5 cursor-pointer text-foreground hover:bg-muted whitespace-nowrap"
      >
        <div className="flex min-w-0 items-center gap-2">
          <BookOpen className="size-4 shrink-0" />
          <span className="min-w-0 flex-1 truncate text-sm">Knowledge bases</span>
        </div>
        <ChevronRight className="size-3 shrink-0 opacity-45" />
      </div>
    </Popover>
  )
}
