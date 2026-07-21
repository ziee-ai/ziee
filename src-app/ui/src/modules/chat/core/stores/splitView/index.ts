import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { splitView, type SplitViewState } from './state'
import type { Actions } from './actions.gen'
import type { PersistedWorkspace } from '../splitWorkspace.persist'
import {
  migrateV1toV2,
  loadWorkspace,
  pruneWorkspace,
  saveWorkspace,
  clearWorkspace,
  isSameTabReload,
} from '../splitWorkspace.persist'

const SplitViewDef = defineStore<SplitViewState, Actions>('SplitView', {
  immer: true,
  state: splitView,
  actions: import.meta.glob('./actions/*.ts'),
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
      void actions.reset()
      void actions.hydrateWorkspace(w)
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
        void applyWorkspace(pruneWorkspace(loaded, () => true))
      } else {
        void actions.reset()
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
      else void actions.reset()
    }

    void (async () => {
      const { useAuthStore } = await import('@/modules/auth/Auth.store')
      applyAuth(useAuthStore.getState().user?.id)
      onCleanup(useAuthStore.subscribe((s) => applyAuth(s.user?.id)))
    })()

    // Debounced save on any layout change. A string fingerprint keeps the
    // subscription from firing on unrelated no-op sets.
    watch(
      SplitViewDef.store,
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
    // A conversation held by a pane was deleted / lost → close that pane so it
    // doesn't sit stale. TWO events cover the two origins (self-echo of the SSE
    // sync stream is suppressed, so the cross-device handler never fires for THIS
    // device's own delete — the local `conversation.deleted` handler covers that):
    const closePaneForConversation = (conversationId: string) => {
      const paneId = get().panes.find(
        (p) => p.conversationId === conversationId,
      )?.paneId
      if (paneId) void actions.closePane(paneId)
    }
    // Cross-device / cross-session delete (arrives via the SSE sync stream).
    on('sync:conversation', (event) => {
      if (event.data.action !== 'delete') return
      closePaneForConversation(event.data.id)
    })
    // LOCAL delete on THIS device (sidebar ⋯ → Delete, project detach, etc.) — the
    // store's `deleteConversation` emits this after the API 200; the SSE echo is
    // self-suppressed, so without this the pane holding it would go stale (FB-23).
    on('conversation.deleted', (event) => {
      closePaneForConversation(event.data.conversationId)
    })
  },
})

export const SplitView = registerLazyStore(SplitViewDef)
export const useSplitViewStore = SplitViewDef.store
