import { defineStore } from '@ziee/framework/store-kit'
import {
  viewDownloadDrawerState,
  type ViewDownloadDrawerState,
} from './state'
import type { Actions } from './actions.gen'

export const ViewDownloadDrawer = defineStore<
  ViewDownloadDrawerState,
  Actions
>('ViewDownloadDrawer', {
  state: viewDownloadDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const useViewDownloadDrawerStore = ViewDownloadDrawer.store
