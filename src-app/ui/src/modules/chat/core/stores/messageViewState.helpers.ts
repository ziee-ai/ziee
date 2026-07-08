/**
 * Pure helpers for the per-conversation message VIEW state (message-scroll-
 * stability, ITEM-6). This state is the ephemeral, per-row UI state that used to
 * live as component-local `useState` INSIDE the virtualized `ChatMessage` row
 * (show-more collapse) and inside `InlineFilePreview` (collapse / seen / resized
 * height). Because those rows unmount when scrolled beyond the virtualizer
 * overscan and remount fresh, the local state was lost on every scroll-away — so
 * it is lifted OUT of the row into a store keyed by a STABLE id (message id for
 * the collapse toggle; the `resource_link` URI for an inline file preview).
 *
 * The math/defaults here are pure + unit-tested (TEST-1); the store
 * (MessageViewState.store.ts) is the thin reactive shell around them.
 */

/** Ephemeral per-inline-file view state (keyed by the resource_link URI). */
export interface InlineFileViewState {
  /** User collapsed the body via the chevron. Default: expanded (false) —
   *  parity with the pre-lift `InlineFilePreview` default. */
  collapsed: boolean
  /** The preview has entered the viewport at least once this session, so its
   *  body may mount immediately on any later remount (no re-lazy-mount height
   *  churn, no re-fetch). Default false so FIRST load still defers off-screen
   *  bodies (DEC-10). */
  seen: boolean
  /** User-chosen body height in px from the drag-resize handle; `null` → use the
   *  viewer's reserved default height (inlineFileHeight.ts). */
  heightPx: number | null
}

/** Show-more default for a long message: collapsed — parity with the pre-lift
 *  `CollapsibleBlock` `useState(true)` default. */
export const DEFAULT_MESSAGE_COLLAPSED = true

/** Inline-file default state (expanded, unseen, reserved-default height). */
export const DEFAULT_INLINE_FILE_STATE: InlineFileViewState = {
  collapsed: false,
  seen: false,
  heightPx: null,
}

/** The two keyed maps the store holds. Kept as a shape so `emptyViewMaps()` is
 *  the single source of the reset value (used by the store's init + reset). */
export interface MessageViewMaps {
  /** message id → collapsed (absent ⇒ DEFAULT_MESSAGE_COLLAPSED). */
  collapsed: Record<string, boolean>
  /** resource_link URI → InlineFileViewState (absent ⇒ DEFAULT_INLINE_FILE_STATE). */
  files: Record<string, InlineFileViewState>
}

/** Fresh, empty maps — the per-conversation reset value (DEC-4). Returns NEW
 *  object identities each call so a reset can never alias the previous maps. */
export function emptyViewMaps(): MessageViewMaps {
  return { collapsed: {}, files: {} }
}

/** Resolve a message's collapsed flag, applying the default for an unknown id. */
export function resolveMessageCollapsed(
  map: Record<string, boolean>,
  messageId: string,
): boolean {
  return map[messageId] ?? DEFAULT_MESSAGE_COLLAPSED
}

/** Resolve an inline file's view state, applying defaults for an unknown key. */
export function resolveFileState(
  map: Record<string, InlineFileViewState>,
  key: string,
): InlineFileViewState {
  return map[key] ?? DEFAULT_INLINE_FILE_STATE
}
