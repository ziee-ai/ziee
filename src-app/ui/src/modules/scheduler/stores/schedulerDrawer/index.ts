import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { schedulerDrawerState, type SchedulerDrawerState } from './state'
import type { Actions } from './actions.gen'

const SchedulerDrawerDef = defineStore<SchedulerDrawerState, Actions>('SchedulerDrawer', {
  immer: true,
  state: schedulerDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const SchedulerDrawer = registerLazyStore(SchedulerDrawerDef)
export const useSchedulerDrawerStore = SchedulerDrawerDef.store
