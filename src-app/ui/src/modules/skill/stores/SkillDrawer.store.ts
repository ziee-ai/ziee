import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { Skill } from '@/api-client/types'

interface SkillDrawerState {
  isOpen: boolean
  skill: Skill | null
  // Optional conversation context — when set, the drawer surfaces the
  // "Hide in this conversation" checkbox.
  conversationId: string | null
  open: (skill: Skill, conversationId?: string) => void
  close: () => void
}

export const useSkillDrawerStore = create<SkillDrawerState>()(
  subscribeWithSelector(
    immer(
      (set): SkillDrawerState => ({
        isOpen: false,
        skill: null,
        conversationId: null,
        open: (skill: Skill, conversationId?: string) =>
          set(draft => {
            draft.isOpen = true
            draft.skill = skill
            draft.conversationId = conversationId ?? null
          }),
        close: () =>
          set(draft => {
            draft.isOpen = false
          }),
      }),
    ),
  ),
)
