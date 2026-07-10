/**
 * Layout limits for the split-chat view (ITEM-14 / DEC-15).
 *
 * FIXED frontend constants, not an admin settings row: the only real operational
 * resource the split consumes is SSE connections, which the SERVER already bounds
 * per-user (chat-stream `PER_USER_MAX_CONNECTIONS`). `MAX_PANES` is a pure
 * client-side ergonomics cap. Structured as a named object (not inline magic
 * numbers) so it can be promoted to configurable later without a rewrite.
 */
export type SplitDirection = 'vertical' | 'horizontal'

export const SPLIT_LIMITS = {
  /** Max simultaneous panes in one window (bounded well under the server's
   *  per-user 12-connection cap). */
  MAX_PANES: 3,
  /** Min/max width (px) of a single pane column when resizing the divider. */
  MIN_PANE_WIDTH: 320,
  MAX_PANE_WIDTH: 1200,
  DEFAULT_DIRECTION: 'vertical' as SplitDirection,
} as const
