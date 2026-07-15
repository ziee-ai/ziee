/**
 * Per-pane composer file OWNERSHIP — the pure logic behind the split-chat file
 * isolation (ITEM-32/ITEM-40), extracted so it is unit-testable WITHOUT importing
 * the 1200-line, enum- and api-client-laden `File.store` (node's strip-only type
 * mode can't load it). Mirrors `mcp/stores/approvalRouting.ts`.
 *
 * The composer buffers (`selectedFiles` / `uploadingFiles`) are shared Maps, but
 * every entry is OWNED by a pane via a parallel owner Map keyed by
 * `composerPaneKey`. Two split panes therefore keep independent attachments: the
 * buffer actions + display filter by the owning pane, and a per-pane backup/
 * restore MERGES only its own entries so a stream-error restore in one pane can
 * never clobber a concurrently-edited other pane's buffer.
 *
 * Every function is pure: it reads the maps it is given and returns NEW maps
 * (immer-safe) — it never mutates an input.
 */

/** Single-pane / primary composer buffer key (ITEM-32). */
export const SINGLE_PANE_KEY = '__single__'

/** Resolve a pane id to a composer buffer key (null/'' → the single-pane key). */
export const composerPaneKey = (paneId: string | null | undefined): string =>
  paneId || SINGLE_PANE_KEY

/**
 * True when the entry `id` belongs to `paneKey`, per its owner map. A missing
 * owner resolves to the single-pane key (via `composerPaneKey`), NOT "unowned" —
 * so the single-pane composer's legacy (owner-less) entries answer to
 * `SINGLE_PANE_KEY`.
 */
export function ownsId(
  ownerMap: Map<string, string>,
  id: string,
  paneKey: string,
): boolean {
  return composerPaneKey(ownerMap.get(id)) === paneKey
}

/** The keys in `entries` owned by `paneKey` (iteration-order preserving). */
export function ownedIds(
  entries: Iterable<string>,
  ownerMap: Map<string, string>,
  paneKey: string,
): string[] {
  return [...entries].filter((id) => ownsId(ownerMap, id, paneKey))
}

/**
 * A NEW map of ONLY `paneKey`'s owned entries — the per-pane backup snapshot.
 * Uses the SAME owner→key resolution as `ownedIds`, so a snapshot captures
 * EXACTLY the entries a paired clear removes.
 */
export function snapshotOwned<V>(
  source: Map<string, V>,
  ownerMap: Map<string, string>,
  paneKey: string,
): Map<string, V> {
  return new Map([...source].filter(([id]) => ownsId(ownerMap, id, paneKey)))
}

/**
 * MERGE `backup`'s entries into `current`, stamping each restored id's owner to
 * `paneKey` — returns NEW maps (immer-safe). A MERGE, not a wholesale replace:
 * a restore for one pane leaves other panes' live entries + owners untouched.
 */
export function mergeOwnedInto<V>(
  current: Map<string, V>,
  ownerMap: Map<string, string>,
  backup: Map<string, V>,
  paneKey: string,
): { next: Map<string, V>; nextOwner: Map<string, string> } {
  const next = new Map(current)
  const nextOwner = new Map(ownerMap)
  for (const [id, v] of backup) {
    next.set(id, v)
    nextOwner.set(id, paneKey)
  }
  return { next, nextOwner }
}
