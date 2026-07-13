import { defineStore } from '@/core/store-kit'
import { SPLIT_LIMITS, type SplitDirection } from '@/modules/chat/core/split/limits'
import {
  openConversationInWorkspace as reconcileOpen,
  type ReconcileIntent,
  type ReconcileOutcome,
} from '@/modules/chat/core/split/reconcile'
import {
  type PersistedWorkspace,
  clearWorkspace,
  isSameTabReload,
  loadWorkspace,
  migrateV1toV2,
  pruneWorkspace,
  saveWorkspace,
} from '@/modules/chat/core/stores/splitWorkspace.persist'

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
 * SplitView workspace store (ITEM-1/24) — the ONLY new global state for the
 * split. Holds the pane LAYOUT (ordered panes, focused pane, per-divider widths,
 * split|tabs mode) as the persistent workspace source of truth; the
 * per-conversation runtime stays per-pane in `ChatPaneStore`. Every "open
 * conversation" request routes through `openConversationInWorkspace` (the pure
 * reconciliation reducer, ITEM-25); persistence is a custom per-user localStorage
 * layer (`splitWorkspace.persist`, ITEM-26), wired in `init`. The URL is a *view
 * into* the workspace (the focused pane's conversation), never its source of
 * truth — v1's `?pane=` mirroring is dropped (DRIFT-1.9).
 */
export const SplitView = defineStore('SplitView', {
  immer: true,
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
      /** Insert the new pane immediately BEFORE this pane (ITEM-70 edge-drop
       *  insert-left). Ignored if `afterPaneId` is also set. */
      beforePaneId?: string
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
        if (opts?.afterPaneId) {
          const idx = d.panes.findIndex((p) => p.paneId === opts.afterPaneId)
          if (idx >= 0) d.panes.splice(idx + 1, 0, pane)
          else d.panes.push(pane)
        } else if (opts?.beforePaneId) {
          const idx = d.panes.findIndex((p) => p.paneId === opts.beforePaneId)
          if (idx >= 0) d.panes.splice(idx, 0, pane)
          else d.panes.push(pane)
        } else {
          d.panes.push(pane)
        }
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

    /** Replace the whole layout from a persisted/pruned workspace (ITEM-26). */
    hydrateWorkspace: (w: {
      panes: Pane[]
      focusedPaneId: string | null
      dividerWidths: number[]
      direction: SplitDirection
      mode: 'split' | 'tabs'
    }) =>
      set((d) => {
        d.panes = w.panes
        d.focusedPaneId = w.focusedPaneId
        d.dividerWidths = w.dividerWidths
        d.direction = w.direction
        d.mode = w.mode
      }),
  }),

  /**
   * Per-user persistence wiring (ITEM-26). Custom (not store-kit `persist`)
   * because the workspace key is `ziee-split-workspace-v2:<userId>` — a shared
   * browser must never restore the previous user's open conversations. Runs only
   * in the browser (the headless unit tests drive the actions directly and never
   * trigger `init`, so they see no persistence side effects).
   *
   * - hydrates the current user's workspace on boot (+ v1→v2 migration),
   *   re-hydrating under the new key on a user switch and clearing on logout;
   * - debounced-saves the layout on every change (a collapsed <2-pane workspace
   *   is removed rather than written — see `saveWorkspace`);
   * - prunes a pane whose conversation is deleted cross-device (`sync:conversation`).
   */
  init: ({ get, on, watch, onCleanup, actions }) => {
    if (typeof window === 'undefined') return

    let currentUserId: string | undefined
    let saveTimer: ReturnType<typeof setTimeout> | null = null

    const snapshot = (): PersistedWorkspace => {
      const s = get()
      return {
        panes: s.panes,
        focusedPaneId: s.focusedPaneId,
        dividerWidths: s.dividerWidths,
        direction: s.direction,
        mode: s.mode,
      }
    }

    const flushSave = () => {
      if (saveTimer) {
        clearTimeout(saveTimer)
        saveTimer = null
      }
      saveWorkspace(currentUserId, snapshot())
    }
    const scheduleSave = () => {
      if (saveTimer) clearTimeout(saveTimer)
      saveTimer = setTimeout(() => {
        saveTimer = null
        saveWorkspace(currentUserId, snapshot())
      }, 250)
    }

    const applyWorkspace = (w: PersistedWorkspace) => {
      actions.reset()
      actions.hydrateWorkspace(w)
    }

    const hydrateFor = (userId: string | undefined) => {
      const migrated = migrateV1toV2(userId)
      const loaded = migrated ?? loadWorkspace(userId)
      // Restore a split ONLY on a same-tab RELOAD (F5). A fresh navigation — a new
      // tab, the ⤢ pop-out (whose sessionStorage is a COPY of the opener's, split
      // and all), a deep link — must start single-pane from the URL, never
      // resurrect that copied/stale split (FB-20 / DEC-74: "open in new tab shows
      // the SAME conversation as the other tab"). Clearing on the fresh-nav path
      // drops the copied blob so a LATER reload of this tab can't restore a foreign
      // split either.
      if (loaded && isSameTabReload()) {
        // Boot prune: drop empty picker panes + collapse <2 to single-pane. The
        // accessibility prune (deleted / no-access conversations) happens on
        // `sync:conversation` delete here + the per-pane 404 auto-close (ITEM-29).
        applyWorkspace(pruneWorkspace(loaded, () => true))
      } else {
        actions.reset()
        clearWorkspace(userId)
      }
    }

    const applyAuth = (userId: string | undefined) => {
      if (userId === currentUserId) return
      // On a user SWITCH flush the previous user's layout under their key first,
      // then re-hydrate (or clear on logout) under the new identity.
      flushSave()
      currentUserId = userId
      if (userId) hydrateFor(userId)
      else actions.reset()
    }

    void (async () => {
      const { useAuthStore } = await import('@/modules/auth/Auth.store')
      applyAuth(useAuthStore.getState().user?.id)
      onCleanup(useAuthStore.subscribe((s) => applyAuth(s.user?.id)))
    })()

    // Debounced save on any layout change. A string fingerprint keeps the
    // subscription from firing on unrelated no-op sets.
    watch(
      SplitView.store,
      (s) =>
        [
          s.mode,
          s.focusedPaneId,
          s.direction,
          s.panes.map((p) => `${p.paneId}:${p.conversationId}`).join(','),
          s.dividerWidths.join(','),
        ].join('|'),
      () => scheduleSave(),
      { fireImmediately: false },
    )
    onCleanup(() => {
      if (saveTimer) clearTimeout(saveTimer)
    })

    // Cross-device delete → drop the pane holding that conversation (ITEM-26).
    on('sync:conversation', (event) => {
      if (event.data.action !== 'delete') return
      const paneId = get().panes.find(
        (p) => p.conversationId === event.data.id,
      )?.paneId
      if (paneId) actions.closePane(paneId)
    })
  },
})

export const useSplitViewStore = SplitView.store
