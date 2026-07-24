import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import {
  viewDownloadDrawerState,
  type ViewDownloadDrawerState,
} from './state'
import type { Actions } from './actions.gen'

const ViewDownloadDrawerDef = defineStore<
  ViewDownloadDrawerState,
  Actions
>('ViewDownloadDrawer', {
  // Draft-mutation actions need immer (see addRemoteLlmModelDrawer note).
  immer: true,
  state: viewDownloadDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const useViewDownloadDrawerStore = ViewDownloadDrawerDef.store

export const ViewDownloadDrawer = registerLazyStore(ViewDownloadDrawerDef)
