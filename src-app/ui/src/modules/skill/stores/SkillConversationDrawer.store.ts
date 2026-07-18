import { defineStore } from '@ziee/framework/store-kit'

/**
 * Visibility for the per-conversation skills panel (chat composer "+").
 *
 * Keyed BY CONVERSATION (not a single global `open`): in a split view the host +
 * menu item render once per pane, so a global boolean opened the drawer on EVERY
 * pane at once (stacked dialogs on different conversations). Tracking WHICH
 * conversation's drawer is open lets each pane render its drawer only when its own
 * conversation is the opened one — mirrors `SkillDrawer.store`'s `conversationId`.
 */
export const SkillConversationDrawer = defineStore('SkillConversationDrawer', {
  immer: true,
  state: { openConversationId: null as string | null },
  actions: set => ({
    openDrawer: (conversationId: string) =>
      set(d => { d.openConversationId = conversationId }),
    closeDrawer: () => set(d => { d.openConversationId = null }),
  }),
})

export const useSkillConversationDrawerStore = SkillConversationDrawer.store
