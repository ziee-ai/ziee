import type { Skill } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'

export const SkillDrawer = defineStore('SkillDrawer', {
  immer: true,
  state: {
    isOpen: false,
    skill: null as Skill | null,
    // Optional conversation context — when set, the drawer surfaces the
    // "Hide in this conversation" checkbox.
    conversationId: null as string | null,
  },
  actions: set => ({
    open: (skill: Skill, conversationId?: string) =>
      set(d => {
        d.isOpen = true
        d.skill = skill
        d.conversationId = conversationId ?? null
      }),
    close: () =>
      set(d => {
        d.isOpen = false
      }),
  }),
})

export const useSkillDrawerStore = SkillDrawer.store
