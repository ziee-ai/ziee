import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { resetPasswordDrawerState, type ResetPasswordDrawerState } from './state'
import type { Actions } from './actions.gen'

const ResetPasswordDrawerDef = defineStore<ResetPasswordDrawerState, Actions>('ResetPasswordDrawer', {
  immer: true,
  state: resetPasswordDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, actions }) => {
    on('user.deleted', event => {
      if (get().user?.id === event.data.userId) actions.closeResetPasswordDrawer()
    })
  },
})
export const ResetPasswordDrawer = registerLazyStore(ResetPasswordDrawerDef)
export const useResetPasswordDrawerStore = ResetPasswordDrawerDef.store
