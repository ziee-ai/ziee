/**
 * Single-pane edge-directional drop (ITEM-57). Dropping a sidebar conversation
 * onto the UNSPLIT conversation view splits by the drop side: the LEFT third
 * opens the dropped conversation as a new pane on the left (`[Y | X]`), the RIGHT
 * third on the right (`[X | Y]`), and the CENTER replaces the current conversation
 * with the dropped one (like a plain sidebar click). Dropping a conversation onto
 * its own open view is a no-op.
 *
 * Both functions are PURE so the geometry + placement are unit-testable without a
 * DOM; `ConversationPage` supplies the pointer x + container rect and applies the
 * resulting plan via the SplitView store.
 */

export type DropZone = 'left' | 'center' | 'right'

/**
 * Classify a pointer x into the left/center/right third of a container. Thirds
 * are `[0, 1/3) → left`, `[1/3, 2/3] → center`, `(2/3, 1] → right`. Clamped, so
 * an x outside the rect still resolves to the nearest edge zone.
 */
export function zoneForX(clientX: number, rectLeft: number, rectWidth: number): DropZone {
  if (rectWidth <= 0) return 'center'
  const frac = (clientX - rectLeft) / rectWidth
  if (frac < 1 / 3) return 'left'
  if (frac > 2 / 3) return 'right'
  return 'center'
}

export type SinglePaneDropPlan =
  | { kind: 'noop' }
  | { kind: 'replace'; id: string }
  | { kind: 'split'; order: [string, string] }

/**
 * Resolve the workspace mutation for dropping `droppedId` onto the single view of
 * `currentId`, by zone. `left` → split `[dropped, current]`; `right` → split
 * `[current, dropped]`; `center` → replace with `dropped`. Dropping a
 * conversation onto its own view is a no-op (never split a conversation with
 * itself, never a redundant self-replace).
 */
export function planSinglePaneDrop(
  zone: DropZone,
  currentId: string,
  droppedId: string,
): SinglePaneDropPlan {
  if (!droppedId || droppedId === currentId) return { kind: 'noop' }
  if (zone === 'center') return { kind: 'replace', id: droppedId }
  if (zone === 'left') return { kind: 'split', order: [droppedId, currentId] }
  return { kind: 'split', order: [currentId, droppedId] }
}
