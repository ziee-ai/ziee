import { SPLIT_LIMITS, type SplitDirection } from '@/modules/chat/core/split/limits'
import type { StoreSet } from '@ziee/framework/store-kit'

/** One split-view pane: a slot holding (at most) one conversation. */
export interface Pane {
  paneId: string
  conversationId: string | null
  projectId: string | null
}

export const splitView = {
  panes: [] as Pane[],
  focusedPaneId: null as string | null,
  /** width (px) of the left pane of each divider gap; index i = gap between pane i and i+1 */
  dividerWidths: [] as number[],
  direction: SPLIT_LIMITS.DEFAULT_DIRECTION as SplitDirection,
  mode: 'split' as 'split' | 'tabs',
  /** Small-screen pane-manager Drawer open-state (ITEM-83). TRANSIENT — deliberately
   *  NOT in `snapshot()`/persist: it's ephemeral UI state, never restored on reload. */
  paneManagerOpen: false,
}

export type SplitViewState = typeof splitView
export type SplitViewSet = StoreSet<SplitViewState>
export type SplitViewGet = () => SplitViewState
