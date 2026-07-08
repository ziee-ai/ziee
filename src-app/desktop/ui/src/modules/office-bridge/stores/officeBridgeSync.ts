import type { OpenDoc } from '@/api-client/types'

// ─────────────────────────────────────────────────────────────────────────────
// Dependency-free core of the OfficeBridge store's notify-and-refetch wiring.
//
// Kept free of runtime `@/…` imports (only a TYPE-only `OpenDoc` import, which is
// erased at build/strip time) so it is unit-testable under the repo's
// `node --test` runner (no bundler / path-alias resolution needed) — the store
// itself pulls in the api-client, zustand, the Chat store, etc. and can't run
// there. See officeBridgeSync.test.ts (TEST-17).
// ─────────────────────────────────────────────────────────────────────────────

/** The right-panel renderer / tab `type` key for the open-documents panel. */
export const OFFICE_DOCS_PANEL_TYPE = 'office-bridge'

/** Stable id for the single "Open Office documents" right-panel tab, so the
 *  tool-result card re-opens the same tab and the store can find + refresh it. */
export const OFFICE_DOCS_PANEL_ID = 'office-bridge-open-documents'

/** The EventBus sync keys the OfficeBridge store refetches on: the owner-scoped
 *  `office_document` open/close notify (DEC-7) plus the blanket `reconnect`
 *  (which fires for every store regardless of audience). */
export const OFFICE_DOCS_SYNC_EVENTS = [
  'sync:office_document',
  'sync:reconnect',
] as const

/** Injected side effects for {@link refetchOpenDocuments} — the store supplies
 *  the real implementations, tests supply fakes. */
export interface RefetchDeps {
  /** True when the user holds `office_bridge::use` (the read perm the
   *  `GET /office-bridge/documents` endpoint enforces). */
  hasUsePermission: () => boolean
  /** Hit `GET /office-bridge/documents`. */
  fetchDocuments: () => Promise<OpenDoc[]>
  /** Expose the refetched list on the store. */
  setDocuments: (docs: OpenDoc[]) => void
  /** Toggle the store's loading flag. */
  setLoading: (loading: boolean) => void
  /** Push the fresh list into the open right-panel tab (no-op when it's closed). */
  pushToOpenPanel?: (docs: OpenDoc[]) => void
  /** Report a fetch failure (best-effort — the panel degrades to its last list). */
  onError?: (err: unknown) => void
}

/**
 * The notify-and-refetch core, **self-gated on `office_bridge::use`** (the no-403
 * rule): `sync:reconnect` fires for every store on every reconnect regardless of
 * the server-side audience, so a user without the read perm must NOT hit the
 * endpoint (it would 403). Returns early without fetching in that case.
 *
 * On a permitted refetch it flips loading, reads the list, exposes it on the
 * store, and pushes it into the open panel tab.
 */
export async function refetchOpenDocuments(deps: RefetchDeps): Promise<void> {
  if (!deps.hasUsePermission()) return
  deps.setLoading(true)
  try {
    const docs = await deps.fetchDocuments()
    deps.setDocuments(docs)
    deps.pushToOpenPanel?.(docs)
  } catch (err) {
    deps.onError?.(err)
  } finally {
    deps.setLoading(false)
  }
}
