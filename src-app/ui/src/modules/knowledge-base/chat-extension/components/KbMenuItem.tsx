import { Popover, message } from '@/components/ui'
import { BookOpen, Check, ChevronRight } from 'lucide-react'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'

/**
 * KbMenuItem — the "+" dropdown row for grounding the conversation on knowledge
 * bases. Opens a submenu (to the right) listing the user's KBs; each row is a
 * TOGGLE (attach/detach persists immediately for a real conversation, or buffers
 * for a not-yet-created one), so the submenu stays open across multiple picks.
 *
 * Mirrors AssistantMenuItem's Popover-list shape, but multi-select instead of
 * single-select. Hidden entirely when the user owns no KBs (nothing to attach).
 */
export function KbMenuItem() {
  // Explicit permission gate (layer 4): no knowledge_base::use → no composer
  // affordance. Defense-in-depth over the store's own load gate (which keeps
  // `items` empty for unpermitted users) so this never depends on load timing.
  const canUse = usePermission(Permissions.KnowledgeBaseUse)
  const { items } = Stores.KnowledgeBases
  const { selectedKbIds } = Stores.KnowledgeBaseComposer

  const kbs = Array.from(items.values())
  if (!canUse || kbs.length === 0) return null

  const toggle = (id: string) => {
    const p = selectedKbIds.has(id)
      ? Stores.KnowledgeBaseComposer.detach(id)
      : Stores.KnowledgeBaseComposer.attach(id)
    // Surface a failed attach/detach instead of a silent unhandled rejection.
    p.catch((e: unknown) =>
      message.error(e instanceof Error ? e.message : 'Failed to update knowledge bases'),
    )
  }

  const popoverContent = (
    <div data-testid="kb-menu-options" style={{ minWidth: 200, margin: -4 }}>
      {kbs.map(kb => {
        const active = selectedKbIds.has(kb.id)
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
            <span className="shrink-0 text-xs text-muted-foreground">
              {kb.document_count}
            </span>
          </div>
        )
      })}
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
