import { defineStore } from '@/core/store-kit'
import { SPLIT_LIMITS, type SplitDirection } from '@/modules/chat/core/split/limits'
import {
  openConversationInWorkspace as reconcileOpen,
  type ReconcileIntent,
  type ReconcileOutcome,
} from '@/modules/chat/core/split/reconcile'

/** One split-view pane: a slot holding (at most) one conversation. */
export interface Pane {
  paneId: string
  conversationId: string | null
  projectId: string | null
}

const newPaneId = (): string =>
  typeof crypto !== 'undefined' && 'randomUUID' in crypto
    ? crypto.randomUUID()
    : `pane-${Date.now()}-${Math.floor(Math.random() * 1e6)}`

/**
 * SplitView store (ITEM-1) — the ONLY new global state for the split. Holds the
 * pane LAYOUT (ordered panes, focused pane, per-divider widths, split|tabs mode);
 * the per-conversation runtime stays per-pane in `ChatPaneStore`. Persists the
 * layout to localStorage (`ziee-split-view-v1`); URL-query mirroring lives in
 * `SplitChatView` (ITEM-8), not the store, so the store stays window-free.
 */
export const SplitView = defineStore('SplitView', {
  immer: true,
  persist: {
    name: 'ziee-split-view-v1',
    partialize: (s) => ({
      panes: s.panes,
      focusedPaneId: s.focusedPaneId,
      dividerWidths: s.dividerWidths,
      direction: s.direction,
      mode: s.mode,
    }),
  },
  state: {
    panes: [] as Pane[],
    focusedPaneId: null as string | null,
    /** width (px) of the left pane of each divider gap; index i = gap between pane i and i+1 */
    dividerWidths: [] as number[],
    direction: SPLIT_LIMITS.DEFAULT_DIRECTION as SplitDirection,
    mode: 'split' as 'split' | 'tabs',
  },
  actions: (set, get) => ({
    /** Open a new pane (optionally seeded with a conversation/project), focus it.
     *  Returns the new paneId, or null if `MAX_PANES` is reached. */
    openPane: (opts?: {
      conversationId?: string | null
      projectId?: string | null
      afterPaneId?: string
    }): string | null => {
      // One conversation per workspace (ITEM-24): opening a conversation already
      // in a pane focuses that pane instead of creating a duplicate.
      if (opts?.conversationId) {
        const existing = get().panes.find(
          (p) => p.conversationId === opts.conversationId,
        )
        if (existing) {
          set((d) => {
            d.focusedPaneId = existing.paneId
          })
          return existing.paneId
        }
      }
      if (get().panes.length >= SPLIT_LIMITS.MAX_PANES) return null
      const paneId = newPaneId()
      const pane: Pane = {
        paneId,
        conversationId: opts?.conversationId ?? null,
        projectId: opts?.projectId ?? null,
      }
      set((d) => {
        const idx = opts?.afterPaneId
          ? d.panes.findIndex((p) => p.paneId === opts.afterPaneId)
          : -1
        if (idx >= 0) d.panes.splice(idx + 1, 0, pane)
        else d.panes.push(pane)
        d.focusedPaneId = paneId
      })
      return paneId
    },

    /** Close a pane; reassign focus ATOMICALLY to a surviving neighbour so
     *  `focusedPaneId` never points at a removed pane (Round-2 finding). */
    closePane: (paneId: string) => {
      set((d) => {
        const idx = d.panes.findIndex((p) => p.paneId === paneId)
        if (idx < 0) return
        d.panes.splice(idx, 1)
        if (d.dividerWidths.length > Math.max(0, d.panes.length - 1)) {
          d.dividerWidths.length = Math.max(0, d.panes.length - 1)
        }
        if (d.focusedPaneId === paneId) {
          const next = d.panes[idx] ?? d.panes[idx - 1] ?? d.panes[0] ?? null
          d.focusedPaneId = next ? next.paneId : null
        }
      })
    },

    focusPane: (paneId: string) => {
      set((d) => {
        if (d.panes.some((p) => p.paneId === paneId)) d.focusedPaneId = paneId
      })
    },

    /** Point a pane at a (different) conversation — the in-pane sidebar switch. */
    setPaneConversation: (
      paneId: string,
      conversationId: string | null,
      projectId: string | null = null,
    ) => {
      // One conversation per workspace (ITEM-24): pointing a pane at a
      // conversation already open in a DIFFERENT pane focuses that pane rather
      // than creating a duplicate. (Adopting a brand-new conversation into its
      // own new-chat pane is unaffected — the conversation is in no other pane.)
      if (conversationId) {
        const dup = get().panes.find(
          (p) => p.conversationId === conversationId && p.paneId !== paneId,
        )
        if (dup) {
          set((d) => {
            d.focusedPaneId = dup.paneId
          })
          return
        }
      }
      set((d) => {
        const p = d.panes.find((pp) => pp.paneId === paneId)
        if (p) {
          p.conversationId = conversationId
          p.projectId = projectId
        }
      })
    },

    reorderPanes: (fromIndex: number, toIndex: number) => {
      set((d) => {
        const n = d.panes.length
        if (fromIndex < 0 || fromIndex >= n || toIndex < 0 || toIndex >= n) return
        const [moved] = d.panes.splice(fromIndex, 1)
        d.panes.splice(toIndex, 0, moved)
      })
    },

    setDividerWidth: (index: number, width: number) => {
      const w = Math.max(
        SPLIT_LIMITS.MIN_PANE_WIDTH,
        Math.min(SPLIT_LIMITS.MAX_PANE_WIDTH, Math.round(width)),
      )
      set((d) => {
        d.dividerWidths[index] = w
      })
    },

    setMode: (mode: 'split' | 'tabs') =>
      set((d) => {
        d.mode = mode
      }),

    /**
     * The v2 workspace entry point (ITEM-24/25) — route EVERY "open conversation"
     * request (sidebar click, ⋯-menu, drag-drop, the router URL effect) through
     * the pure reconciliation reducer so the workspace behaves identically no
     * matter how the conversation was opened, and never duplicates a conversation
     * across panes. Applies the reducer's `next` layout and returns its `outcome`
     * so the caller can do the impure follow-up (navigate / toast / offer-replace).
     */
    openConversationInWorkspace: (
      conversationId: string,
      intent: ReconcileIntent,
      opts?: { currentConversationId?: string | null; projectId?: string | null },
    ): ReconcileOutcome => {
      const { next, outcome } = reconcileOpen({
        layout: {
          panes: get().panes,
          focusedPaneId: get().focusedPaneId,
        },
        currentConversationId: opts?.currentConversationId ?? null,
        conversationId,
        projectId: opts?.projectId ?? null,
        intent,
        newPaneId,
      })
      set((d) => {
        d.panes = next.panes
        d.focusedPaneId = next.focusedPaneId
        // Drop any divider widths beyond the surviving pane-gap count.
        if (d.dividerWidths.length > Math.max(0, d.panes.length - 1)) {
          d.dividerWidths.length = Math.max(0, d.panes.length - 1)
        }
      })
      return outcome
    },

    reset: () =>
      set((d) => {
        d.panes = []
        d.focusedPaneId = null
        d.dividerWidths = []
        d.mode = 'split'
      }),
  }),
})

export const useSplitViewStore = SplitView.store
