/**
 * Pane drag-and-drop payloads + classification (ITEM-31).
 *
 * Two custom drag types keep the workspace drop-zones from ever cross-firing
 * with the composer's OS-**file** drop: a conversation/pane drop-zone only acts
 * on ITS type and explicitly yields to a `Files` drag (which belongs to the
 * composer as an attachment). The zones themselves live on the pane HEADER + the
 * inter-pane SEAM — never over the composer — so the two never physically
 * overlap either. `dragKind` reads `dataTransfer.types` (the only thing readable
 * during `dragover`; `getData` is blocked until `drop`).
 */

export const CONVERSATION_DND_TYPE = 'application/x-ziee-conversation'
export const PANE_DND_TYPE = 'application/x-ziee-pane'

export type DragKind = 'conversation' | 'pane' | 'file'

/**
 * Classify an in-flight drag from `dataTransfer.types`. A `Files` drag wins
 * nothing here (returns `'file'`) so a conversation/pane drop-zone IGNORES it —
 * the disambiguation TEST-28 asserts. Bias is toward `'file'`: if a drag somehow
 * carried both, it is treated as a file (never a pane swap).
 */
export function dragKind(dt: Pick<DataTransfer, 'types'>): DragKind | null {
  const types = Array.from(dt.types)
  if (types.includes('Files')) return 'file'
  if (types.includes(CONVERSATION_DND_TYPE)) return 'conversation'
  if (types.includes(PANE_DND_TYPE)) return 'pane'
  return null
}

/** True for a drag a workspace drop-zone should accept (conversation or pane). */
export function isWorkspaceDrag(dt: Pick<DataTransfer, 'types'>): boolean {
  const k = dragKind(dt)
  return k === 'conversation' || k === 'pane'
}

export function setConversationDragData(
  dt: DataTransfer,
  conversationId: string,
): void {
  dt.setData(CONVERSATION_DND_TYPE, conversationId)
  dt.effectAllowed = 'copyMove'
}

export function setPaneDragData(dt: DataTransfer, paneId: string): void {
  dt.setData(PANE_DND_TYPE, paneId)
  dt.effectAllowed = 'move'
}

export function readConversationDragId(dt: DataTransfer): string | null {
  return dt.getData(CONVERSATION_DND_TYPE) || null
}

export function readPaneDragId(dt: DataTransfer): string | null {
  return dt.getData(PANE_DND_TYPE) || null
}

/**
 * Resolve the `reorderPanes(from, to)` index pair for a header-drag from
 * `fromPaneId` onto `toPaneId`, against the current pane order. Returns null when
 * either id is unknown or they're identical (a no-op). Pure — the caller applies
 * it via the store.
 */
export function reorderIndices(
  panes: ReadonlyArray<{ paneId: string }>,
  fromPaneId: string,
  toPaneId: string,
): { from: number; to: number } | null {
  if (fromPaneId === toPaneId) return null
  const from = panes.findIndex((p) => p.paneId === fromPaneId)
  const to = panes.findIndex((p) => p.paneId === toPaneId)
  if (from < 0 || to < 0) return null
  return { from, to }
}
