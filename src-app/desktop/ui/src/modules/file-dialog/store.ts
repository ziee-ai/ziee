/**
 * File Dialog Store — native file picker dialogs via @tauri-apps/plugin-dialog.
 */
import { open, save } from '@tauri-apps/plugin-dialog'
import { defineStore } from '@ziee/framework/store-kit'
import { type StoreProxy } from '@ziee/framework/stores'

interface FileFilter {
  name: string
  extensions: string[]
}

export const FileDialog = defineStore('FileDialog', {
  state: {},
  actions: () => ({
    openFile: async (options?: {
      title?: string
      filters?: FileFilter[]
      multiple?: boolean
    }): Promise<string | string[] | null> => {
      try {
        return await open({
          title: options?.title ?? 'Select File',
          filters: options?.filters,
          multiple: options?.multiple ?? false,
          directory: false,
        })
      } catch (error) {
        console.error('Failed to open file dialog:', error)
        return null
      }
    },
    openFolder: async (options?: {
      title?: string
      multiple?: boolean
    }): Promise<string | string[] | null> => {
      try {
        return await open({
          title: options?.title ?? 'Select Folder',
          multiple: options?.multiple ?? false,
          directory: true,
        })
      } catch (error) {
        console.error('Failed to open folder dialog:', error)
        return null
      }
    },
    saveFile: async (options?: {
      title?: string
      defaultPath?: string
      filters?: FileFilter[]
    }): Promise<string | null> => {
      try {
        return await save({
          title: options?.title ?? 'Save File',
          defaultPath: options?.defaultPath,
          filters: options?.filters,
        })
      } catch (error) {
        console.error('Failed to open save dialog:', error)
        return null
      }
    },
  }),
})

export const useFileDialogStore = FileDialog.store

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    FileDialog: StoreProxy<ReturnType<typeof FileDialog.store.getState>>
  }
}
