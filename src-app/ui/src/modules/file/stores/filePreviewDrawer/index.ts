import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { filePreviewDrawerState, type FilePreviewDrawerState } from './state'
import type { Actions } from './actions.gen'

const FilePreviewDrawerDef = defineStore<FilePreviewDrawerState, Actions>('FilePreviewDrawer', {
  immer: true,
  state: filePreviewDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const FilePreviewDrawer = registerLazyStore(FilePreviewDrawerDef)
export const useFilePreviewDrawerStore = FilePreviewDrawerDef.store
