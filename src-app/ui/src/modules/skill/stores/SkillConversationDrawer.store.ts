import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'

/**
 * Visibility state for the per-conversation skills panel opened from the
 * chat composer's "+" dropdown. Mirrors McpComposer's
 * `configModalVisible` pattern so the menu item can render the drawer
 * inline and toggle it.
 */
interface SkillConversationDrawerState {
  open: boolean
  openDrawer: () => void
  closeDrawer: () => void
}

export const useSkillConversationDrawerStore =
  create<SkillConversationDrawerState>()(
    subscribeWithSelector(
      immer(
        (set): SkillConversationDrawerState => ({
          open: false,
          openDrawer: () =>
            set(draft => {
              draft.open = true
            }),
          closeDrawer: () =>
            set(draft => {
              draft.open = false
            }),
        }),
      ),
    ),
  )
