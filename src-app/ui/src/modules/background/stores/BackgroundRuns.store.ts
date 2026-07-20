import { ApiClient } from '@/api-client'
import {
  type BackgroundRunCancelAck,
  type BackgroundRunSummary,
  type RunNote,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

/**
 * `background::use` gates the `/api/background/runs` surface (backend
 * `BackgroundUse`, granted to the default Users group by migration
 * `202607191000_background_grant_permissions.sql`). It is present in
 * `openapi.json` but the current `api-client/types.ts` `Permissions` enum was
 * regenerated WITHOUT the entry (a regen gap — the enum jumps
 * `AuthProvidersRead` → `BranchesCreate`). Until a regen adds
 * `Permissions.BackgroundUse`, the leaf is spelled here as the raw permission
 * string, which `hasPermissionNow` and the slot/route `permission` fields accept
 * (framework `PermissionExpr` = bare `string`). Swap this for
 * `Permissions.BackgroundUse` once regen lands.
 */
export const BACKGROUND_USE_PERMISSION = 'background::use'

/**
 * Terminal run statuses (mirrors the backend `WorkflowRunStatus::is_terminal`).
 * Cancel and steer both 409 on a terminal run, so both affordances are gated on
 * `!isTerminalRunStatus(status)`.
 */
const TERMINAL_STATUSES = new Set(['completed', 'failed', 'cancelled'])
export const isTerminalRunStatus = (status: string): boolean =>
  TERMINAL_STATUSES.has(status)

/**
 * The user's background sub-agent / sandbox-exec runs (ITEM-8). Server-paginated
 * over `GET /api/background/runs`; refetches live on `sync:workflow_run` (the
 * backbone emits it — `Audience::owner` — on every background-run state change)
 * so statuses move to their terminal badge without a manual reload.
 *
 * Mirrors `McpToolCalls.store` (paginated + sync-subscribed + self-gated).
 */
export const BackgroundRuns = defineStore('BackgroundRuns', {
  immer: true,
  state: {
    runs: [] as BackgroundRunSummary[],
    total: 0,
    currentPage: 1,
    pageSize: 10,
    loading: false,
    error: null as string | null,
    /**
     * Pending steering notes keyed by run id, loaded on demand when a row's
     * steer composer is opened (avoids an N-fetch fan-out across the page).
     */
    notesByRun: {} as Record<string, RunNote[]>,
  },
  actions: (set, get) => {
    const loadRuns = async (page?: number, pageSize?: number): Promise<void> => {
      // no-403 invariant: gate on the SAME permission the endpoint enforces.
      if (!hasPermissionNow(BACKGROUND_USE_PERMISSION)) return
      const state = get()
      const nextPage = page ?? state.currentPage
      const nextPageSize = pageSize ?? state.pageSize
      try {
        set(draft => {
          draft.loading = true
          draft.error = null
        })
        const response = await ApiClient.Background.listRuns({
          page: nextPage,
          per_page: nextPageSize,
        })
        set(draft => {
          draft.runs = response.runs
          draft.total = response.total
          draft.currentPage = response.page
          draft.pageSize = response.per_page
          draft.loading = false
        })
      } catch (error) {
        console.error('Background runs load failed:', error)
        set(draft => {
          draft.loading = false
          draft.error =
            error instanceof Error ? error.message : 'Failed to load background tasks'
        })
      }
    }

    const loadNotes = async (runId: string): Promise<void> => {
      if (!hasPermissionNow(BACKGROUND_USE_PERMISSION)) return
      try {
        const notes = await ApiClient.Background.listRunNotes({ run_id: runId })
        set(draft => {
          draft.notesByRun[runId] = notes
        })
      } catch (error) {
        // Non-fatal: the steer composer still works without the pending list.
        console.error('Background run notes load failed:', error)
      }
    }

    return {
      loadRuns,
      loadNotes,
      setPage: (page: number, pageSize?: number): void => {
        void loadRuns(page, pageSize)
      },
      /**
       * Cancel a non-terminal run. The server flips the row + emits
       * `sync:workflow_run` (→ the row refreshes to `cancelled`); we also refetch
       * the current page immediately as a backstop. Throws on failure so the UI
       * layer toasts it (the store carries no per-mutation error state).
       */
      cancelRun: async (runId: string): Promise<BackgroundRunCancelAck> => {
        const ack = await ApiClient.Background.cancelRun({ run_id: runId })
        await loadRuns()
        return ack
      },
      /**
       * Queue a steering note to a non-terminal run. Throws (e.g. 409 on a run
       * that finished between render and submit) so the UI layer toasts it;
       * refreshes that run's pending-note list on success.
       */
      postNote: async (runId: string, note: string): Promise<RunNote> => {
        const created = await ApiClient.Background.postRunNote({
          run_id: runId,
          note,
        })
        await loadNotes(runId)
        return created
      },
      clearError: (): void =>
        set(draft => {
          draft.error = null
        }),
    }
  },
  init: ({ on, get, actions }) => {
    // Live refresh: refetch the current page on any owner-scoped background-run
    // state change and on SSE reconnect. Self-gated inside `loadRuns`
    // (no-403-on-reconnect for a role without `background::use`).
    const reload = (): void => {
      void actions.loadRuns(get().currentPage)
    }
    on('sync:workflow_run', reload)
    on('sync:reconnect', reload)
  },
})

export const useBackgroundRunsStore = BackgroundRuns.store
