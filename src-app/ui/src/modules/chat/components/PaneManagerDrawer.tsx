import { useEffect, useMemo, useRef, useState } from 'react'
import { MessageSquarePlus, Search, X } from 'lucide-react'
import { Button, Empty, Input, Tooltip } from '@ziee/kit'
import { cn } from '@/lib/utils'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'
import {
  useClosePane,
  useOpenConversationInWorkspace,
} from '@/modules/chat/core/pane/useOpenConversation'
import { ChatHistory } from '@/modules/chat/stores/chatHistory'
import { SplitView as SplitViewStore } from '@/modules/chat/core/stores/splitView'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/**
 * Small-screen pane manager (ITEM-83 / FB-26). Below the `md` breakpoint the split
 * can't tile columns, so the tab strip + drag affordances are removed and a focused
 * pane reads as a normal single-pane conversation. This Drawer — opened from the
 * pane header's "Panes" button — is the single place to
 *
 *   1. see the conversations open in this window's panes and tap one to FOCUS it, and
 *   2. open ANOTHER conversation into a new pane (searchable list + New chat).
 *
 * It REPLACES the always-visible mobile tab strip. Open-state is the transient
 * `SplitViewStore.paneManagerOpen` (never persisted). Rendered once in the chat
 * route brancher, so it's reachable from both the single-pane and split headers.
 */
export function PaneManagerDrawer() {
  const { panes, focusedPaneId, paneManagerOpen } = SplitViewStore
  const { conversations, isInitialized } = ChatHistory
  // The single-pane route's conversation (0 SplitView panes) — read reactively off
  // the focused-pane bridge, which resolves to the primary pane outside a split.
  const primaryConvId = Chat.conversation?.id ?? null

  const [query, setQuery] = useState('')
  const closePaneHook = useClosePane()
  const openInWorkspace = useOpenConversationInWorkspace()
  const openListRef = useRef<HTMLUListElement>(null)

  useEffect(() => {
    if (paneManagerOpen && !isInitialized) ChatHistory.loadConversations()
  }, [paneManagerOpen, isInitialized])

  // Auto-dismiss on a LINGERING collapse (exactly 1 pane): a valid open state is
  // either the single-pane route (0 SplitView panes) or a real split (≥2). Exactly 1
  // pane only arises when a split collapses WITHOUT going through the close hook —
  // e.g. a cross-device `sync:conversation` delete of a pane's conversation drops the
  // pane via `store.closePane` (no `reset()`), leaving one pane + this right-placement
  // drawer stranded over the survivor. Close it so the survivor is visible.
  useEffect(() => {
    if (paneManagerOpen && panes.length === 1) {
      SplitViewStore.setPaneManagerOpen(false)
    }
  }, [paneManagerOpen, panes.length])

  const close = () => SplitViewStore.setPaneManagerOpen(false)

  // Close a pane through the shared hook (collapse-to-single + navigate to the
  // survivor). If the workspace collapses to a single pane, dismiss the manager so
  // the surviving conversation is shown (else the full-bleed drawer would cover it).
  // Otherwise the drawer STAYS open and the closed row unmounts — Radix only restores
  // focus to the trigger on a full close, so move keyboard focus to the first
  // surviving row here (WCAG 2.4.3, in-list removal).
  const closePane = (paneId: string) => {
    closePaneHook(paneId)
    if (SplitViewStore.$.panes.length < 2) {
      close()
      return
    }
    requestAnimationFrame(() => {
      openListRef.current?.querySelector<HTMLButtonElement>('button')?.focus()
    })
  }

  const titleFor = (id: string | null): string =>
    id
      ? conversations.find((c) => c.id === id)?.title || 'Conversation'
      : 'New chat'

  // Conversations currently open in this window. A split (≥1 pane) → the pane list;
  // the single-pane route (0 panes) → the one route conversation (focus is a no-op,
  // it's already showing, so it just closes the drawer).
  const openEntries = useMemo(() => {
    if (panes.length > 0) {
      return panes.map((p) => ({
        key: p.paneId,
        paneId: p.paneId as string | null,
        title: titleFor(p.conversationId),
        active: p.paneId === focusedPaneId,
        closable: true,
      }))
    }
    if (primaryConvId) {
      return [
        {
          key: primaryConvId,
          paneId: null as string | null,
          title: titleFor(primaryConvId),
          active: true,
          closable: false,
        },
      ]
    }
    return []
    // titleFor is a pure closure over `conversations`, already a dep.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [panes, focusedPaneId, conversations, primaryConvId])

  // A conversation already open in this window can't be opened again (one per
  // workspace) — hide the open ones from the "open another" list.
  const openIds = useMemo(() => {
    const ids = new Set<string>()
    for (const p of panes) if (p.conversationId) ids.add(p.conversationId)
    if (primaryConvId) ids.add(primaryConvId)
    return ids
  }, [panes, primaryConvId])

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    return conversations.filter((c) => {
      if (openIds.has(c.id)) return false
      if (!q) return true
      return (c.title || 'Untitled Conversation').toLowerCase().includes(q)
    })
  }, [conversations, openIds, query])

  const atCap = panes.length >= SPLIT_LIMITS.MAX_PANES

  const focusEntry = (paneId: string | null) => {
    if (paneId) SplitViewStore.focusPane(paneId)
    close()
  }

  // Seed the single-pane route's conversation into pane 0 BEFORE adding a new pane,
  // so the first "open another" from a lone conversation yields a real 2-pane split
  // (mirrors `onSplit`'s bootstrap in ConversationPage).
  const seedIfSingle = () => {
    if (SplitViewStore.$.panes.length === 0 && primaryConvId) {
      SplitViewStore.openPane({ conversationId: primaryConvId })
    }
  }

  const openAnother = (id: string) => {
    // Route through the canonical reconcile path (`useOpenConversationInWorkspace`)
    // rather than hand-rolling the bootstrap: the explicit `newPane` intent seeds the
    // base from the current single-pane conversation (the hook's `currentConversationId`
    // = focused pane ?? URL), skips the placement dialog, and navigates — so this shares
    // the ONE open path with the sidebar / drag / menu (and gets pop-out dedup + the
    // MAX_PANES offer-replace for free), instead of a third copy of the seed logic.
    void openInWorkspace(id, { intent: 'newPane' })
    setQuery('')
    close()
  }

  const newChat = () => {
    seedIfSingle()
    SplitViewStore.openPane({ conversationId: null }) // empty picker pane, focused
    setQuery('')
    close()
  }

  return (
    <Drawer
      open={paneManagerOpen}
      onClose={close}
      title="Panes"
      placement="right"
      data-testid="pane-manager-drawer"
    >
      <div className="flex w-full flex-col gap-5 py-1">
        {/* Open panes — tap a row to focus it; ✕ closes it. */}
        <section className="flex flex-col gap-1">
          <div className="px-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Open panes
          </div>
          <ul className="flex flex-col" data-testid="pane-manager-open-list">
            {openEntries.map((e) => (
              <li
                key={e.key}
                // The focus row + its close ✕ touch to read as one item — the
                // zero-gap is intended (segmented-control idiom, like the old tab).
                data-allow-adjacent
                className={cn(
                  'flex items-center rounded-md',
                  e.active && 'bg-accent',
                )}
              >
                <Button
                  variant="ghost"
                  data-testid={`pane-manager-focus-${e.paneId ?? 'current'}`}
                  aria-current={e.active ? 'true' : undefined}
                  className={cn(
                    'h-auto min-w-0 flex-1 justify-start gap-2 px-2 py-2 font-normal',
                    // Persistent selected state (not a hover) → pair bg-accent with
                    // accent-foreground per the shadcn selected-item pattern.
                    e.active && 'text-accent-foreground',
                  )}
                  onClick={() => focusEntry(e.paneId)}
                >
                  <span className="min-w-0 flex-1 truncate text-start text-sm">
                    {e.title}
                  </span>
                  {e.active && (
                    <span className="shrink-0 text-xs text-muted-foreground">
                      Viewing
                    </span>
                  )}
                </Button>
                {e.closable && e.paneId && (
                  <Tooltip content="Close pane">
                    <Button
                      variant="ghost"
                      size="icon"
                      icon={<X />}
                      aria-label="Close pane"
                      data-testid={`pane-manager-close-${e.paneId}`}
                      className="size-9 shrink-0"
                      onClick={() => closePane(e.paneId as string)}
                    />
                  </Tooltip>
                )}
              </li>
            ))}
          </ul>
        </section>

        {/* Open another conversation into a new pane. */}
        <section className="flex flex-col gap-2">
          <div className="px-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Open another
          </div>
          <Button
            data-testid="pane-manager-new-chat"
            variant="outline"
            className="w-full justify-start"
            icon={<MessageSquarePlus />}
            disabled={atCap}
            onClick={newChat}
          >
            New chat
          </Button>
          <Input
            data-testid="pane-manager-search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search conversations..."
            prefix={<Search className="size-4 text-muted-foreground" />}
            aria-label="Search conversations"
            disabled={atCap}
          />
          <div data-testid="pane-manager-list">
            {atCap ? (
              <Empty
                data-testid="pane-manager-cap"
                className="py-8"
                description={`Maximum ${SPLIT_LIMITS.MAX_PANES} panes open`}
              />
            ) : filtered.length === 0 ? (
              <Empty
                data-testid="pane-manager-empty"
                className="py-8"
                description={
                  conversations.length === 0
                    ? 'No conversations yet'
                    : 'No conversations match your search'
                }
              />
            ) : (
              <ul className="flex flex-col">
                {filtered.map((c) => (
                  <li key={c.id}>
                    <Button
                      variant="ghost"
                      data-testid={`pane-manager-open-${c.id}`}
                      className="h-auto w-full justify-start gap-2 px-2 py-2 font-normal"
                      onClick={() => openAnother(c.id)}
                    >
                      <span className="min-w-0 flex-1 truncate text-start text-sm">
                        {c.title || 'Untitled Conversation'}
                      </span>
                    </Button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </section>
      </div>
    </Drawer>
  )
}
