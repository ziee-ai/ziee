/**
 * File Dialog Store
 *
 * Provides hooks for native file picker dialogs using @tauri-apps/plugin-dialog
 */

import { create } from 'zustand'
import { open, save } from '@tauri-apps/plugin-dialog'
import { type StoreProxy } from '@/core/stores'

interface FileFilter {
  name: string
  extensions: string[]
}

interface FileDialogState {
  // Actions
  openFile: (options?: {
    title?: string
    filters?: FileFilter[]
    multiple?: boolean
  }) => Promise<string | string[] | null>
  openFolder: (options?: {
    title?: string
    multiple?: boolean
  }) => Promise<string | string[] | null>
  saveFile: (options?: {
    title?: string
    defaultPath?: string
    filters?: FileFilter[]
  }) => Promise<string | null>
}

declare module '@/core/stores' {
  interface RegisteredStores {
    FileDialog: StoreProxy<FileDialogState>
  }
}

export const useFileDialogStore = create<FileDialogState>(() => ({
  openFile: async (options) => {
    try {
      const result = await open({
        title: options?.title ?? 'Select File',
        filters: options?.filters,
        multiple: options?.multiple ?? false,
        directory: false,
      })
      return result
    } catch (error) {
      console.error('Failed to open file dialog:', error)
      return null
    }
  },

  openFolder: async (options) => {
    try {
      const result = await open({
        title: options?.title ?? 'Select Folder',
        multiple: options?.multiple ?? false,
        directory: true,
      })
      return result
    } catch (error) {
      console.error('Failed to open folder dialog:', error)
      return null
    }
  },

  saveFile: async (options) => {
    try {
      const result = await save({
        title: options?.title ?? 'Save File',
        defaultPath: options?.defaultPath,
        filters: options?.filters,
      })
      return result
    } catch (error) {
      console.error('Failed to open save dialog:', error)
      return null
    }
  },
}))
