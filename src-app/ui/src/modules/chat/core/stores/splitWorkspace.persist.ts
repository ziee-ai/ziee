import type { Pane } from '@/modules/chat/core/stores/SplitView.store'
import type { SplitDirection } from '@/modules/chat/core/split/limits'

/**
 * Per-user workspace persistence (ITEM-26 / DEC-42/51) — a CUSTOM localStorage
 * layer, NOT store-kit's `persist` middleware, because the split workspace must
 * be keyed by the logged-in user (`ziee-split-workspace-v2:<userId>`): a shared
 * browser must never restore the previous user's open conversations (localStorage
 * survives logout), mirroring `chatDrafts.makeDraftKey`'s per-user namespacing.
 *
 * The pure `pruneWorkspace` + `migrateV1toV2` helpers are unit-tested directly
 * (TEST-48); the `SplitView` store's `init` wires them to the auth lifecycle.
 */

const PREFIX = 'ziee-split-workspace-v2:'
/** The v1 store-kit-persist key this module migrates from (single, un-namespaced). */
const V1_KEY = 'ziee-split-view-v1'

/** The persisted slice of the workspace (the `SplitView` partialize shape). */
export interface PersistedWorkspace {
  panes: Pane[]
  focusedPaneId: string | null
  dividerWidths: number[]
  direction: SplitDirection
  mode: 'split' | 'tabs'
}

/** localStorage key for a user's workspace. `anon` is a defensive fallback. */
export function workspaceStorageKey(userId: string | null | undefined): string {
  return `${PREFIX}${userId ?? 'anon'}`
}

/** A workspace that is fully collapsed to single-pane (URL-driven). */
function emptyWorkspace(base?: Partial<PersistedWorkspace>): PersistedWorkspace {
  return {
    panes: [],
    focusedPaneId: null,
    dividerWidths: [],
    direction: base?.direction ?? 'vertical',
    mode: base?.mode ?? 'split',
  }
}

/** Best-effort shape guard for a parsed localStorage blob. */
function isWorkspaceLike(v: unknown): v is PersistedWorkspace {
  return (
    typeof v === 'object' &&
    v !== null &&
    Array.isArray((v as { panes?: unknown }).panes)
  )
}

/** Read + parse a user's persisted workspace (or null if none / unavailable). */
export function loadWorkspace(
  userId: string | null | undefined,
): PersistedWorkspace | null {
  try {
    const raw = localStorage.getItem(workspaceStorageKey(userId))
    if (!raw) return null
    const parsed = JSON.parse(raw)
    if (!isWorkspaceLike(parsed)) return null
    return {
      panes: parsed.panes,
      focusedPaneId: parsed.focusedPaneId ?? null,
      dividerWidths: Array.isArray(parsed.dividerWidths)
        ? parsed.dividerWidths
        : [],
      direction: parsed.direction ?? 'vertical',
      mode: parsed.mode === 'tabs' ? 'tabs' : 'split',
    }
  } catch {
    return null
  }
}

/**
 * Save a user's workspace. A collapsed (single-pane) workspace is REMOVED rather
 * than written, so a user who exits the split doesn't leave a stale blob that
 * would re-expand a lone conversation on next boot.
 */
export function saveWorkspace(
  userId: string | null | undefined,
  ws: PersistedWorkspace,
): void {
  try {
    if (ws.panes.length < 2) {
      localStorage.removeItem(workspaceStorageKey(userId))
      return
    }
    localStorage.setItem(workspaceStorageKey(userId), JSON.stringify(ws))
  } catch {
    // best-effort — private mode / quota degrade to no persistence.
  }
}

/** Remove a user's persisted workspace. */
export function clearWorkspace(userId: string | null | undefined): void {
  try {
    localStorage.removeItem(workspaceStorageKey(userId))
  } catch {
    // ignore
  }
}

/**
 * Prune a workspace to only accessible, non-empty panes (PURE — TEST-48):
 *
 * - drop panes whose conversation is deleted / not-accessible (`!isAccessible`),
 * - drop empty (picker / `conversationId:null`) panes — not worth restoring,
 * - if fewer than 2 panes survive, COLLAPSE to single-pane (URL-driven) so a
 *   lone conversation is reached via the URL, not a degenerate 1-pane split,
 * - clamp `focusedPaneId` to a survivor and `dividerWidths` to the gap count.
 */
export function pruneWorkspace(
  ws: PersistedWorkspace,
  isAccessible: (conversationId: string) => boolean,
): PersistedWorkspace {
  const panes = ws.panes.filter(
    (p): p is Pane & { conversationId: string } =>
      p.conversationId !== null && isAccessible(p.conversationId),
  )
  if (panes.length < 2) return emptyWorkspace(ws)
  const focusedPaneId = panes.some((p) => p.paneId === ws.focusedPaneId)
    ? ws.focusedPaneId
    : panes[0].paneId
  return {
    panes,
    focusedPaneId,
    dividerWidths: ws.dividerWidths.slice(0, Math.max(0, panes.length - 1)),
    direction: ws.direction,
    mode: ws.mode,
  }
}

/**
 * One-time v1→v2 migration: if the old un-namespaced `ziee-split-view-v1` key
 * exists, move its layout under the per-user v2 key and DELETE the v1 key (so it
 * migrates exactly once, and can't leak across users on a shared browser).
 * Returns the migrated workspace, or null if there was nothing to migrate.
 */
export function migrateV1toV2(
  userId: string | null | undefined,
): PersistedWorkspace | null {
  try {
    const raw = localStorage.getItem(V1_KEY)
    if (!raw) return null
    localStorage.removeItem(V1_KEY)
    const parsed = JSON.parse(raw)
    // store-kit persist wraps the payload as `{ state, version }`.
    const inner =
      parsed && typeof parsed === 'object' && 'state' in parsed
        ? (parsed as { state: unknown }).state
        : parsed
    if (!isWorkspaceLike(inner)) return null
    const ws: PersistedWorkspace = {
      panes: inner.panes,
      focusedPaneId: inner.focusedPaneId ?? null,
      dividerWidths: Array.isArray(inner.dividerWidths)
        ? inner.dividerWidths
        : [],
      direction: inner.direction ?? 'vertical',
      mode: inner.mode === 'tabs' ? 'tabs' : 'split',
    }
    saveWorkspace(userId, ws)
    return ws
  } catch {
    return null
  }
}
