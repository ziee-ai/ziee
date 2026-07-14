import { defineStore } from '@ziee/framework/store-kit'

/** Visibility for the per-conversation skills panel (chat composer "+"). */
export const SkillConversationDrawer = defineStore('SkillConversationDrawer', {
  immer: true,
  state: { open: false },
  actions: set => ({
    openDrawer: () => set(d => { d.open = true }),
    closeDrawer: () => set(d => { d.open = false }),
  }),
})

export const useSkillConversationDrawerStore = SkillConversationDrawer.store
