import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { fileRagAdminState, type FileRagAdminState } from './state'
import type { Actions } from './actions.gen'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'

const FileRagAdminDef = defineStore<FileRagAdminState, Actions>('FileRagAdmin', {
  immer: true,
  state: fileRagAdminState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.FileRagAdminRead)) return
      void actions.load()
      void actions.loadEmbeddingModels()
      void actions.loadRerankerModels()
    }
    on('sync:file_rag_admin_settings', reload)
    on('sync:reconnect', reload)
    if (hasPermissionNow(Permissions.FileRagAdminRead)) {
      void actions.load()
      void actions.loadEmbeddingModels()
      void actions.loadRerankerModels()
    }
  },
})

export const FileRagAdmin = registerLazyStore(FileRagAdminDef)
/** Raw store reference for gallery imperative state manipulation. */
export const FileRagAdminStore = FileRagAdminDef.store
export const useFileRagAdminStore = FileRagAdminDef.store
