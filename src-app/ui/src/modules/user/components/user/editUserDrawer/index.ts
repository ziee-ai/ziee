import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { editUserDrawerState, type EditUserDrawerState } from './state'
import type { Actions } from './actions.gen'

const EditUserDrawerDef = defineStore<EditUserDrawerState, Actions>(
  'EditUserDrawer',
  {
    immer: true,
    state: editUserDrawerState,
    actions: import.meta.glob('./actions/*.ts'),
    init: ({ on, get, set, actions }) => {
      on('user.updated', event => {
        if (get().editingUser?.id === event.data.user.id) {
          set({ editingUser: event.data.user })
        }
      })
      on('user.deleted', event => {
        if (get().editingUser?.id === event.data.userId) {
          void actions.closeEditUserDrawer()
        }
      })
    },
  },
)

export const EditUserDrawer = registerLazyStore(EditUserDrawerDef)
export const useEditUserDrawerStore = EditUserDrawerDef.store
