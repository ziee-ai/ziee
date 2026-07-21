import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { createUserDrawerState, type CreateUserDrawerState } from './state'
import type { Actions } from './actions.gen'

const CreateUserDrawerDef = defineStore<CreateUserDrawerState, Actions>(
  'CreateUserDrawer',
  {
    immer: true,
    state: createUserDrawerState,
    actions: import.meta.glob('./actions/*.ts'),
  },
)

export const CreateUserDrawer = registerLazyStore(CreateUserDrawerDef)
export const useCreateUserDrawerStore = CreateUserDrawerDef.store
