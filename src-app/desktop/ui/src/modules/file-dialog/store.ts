/**
 * File Dialog Store
 *
 * Provides hooks for native file picker dialogs
 */

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

interface FileFilter {
  name: string
  extensions: string[]
}

interface FileDialogState {
  // Actions
  openFile: (filters?: FileFilter[]) => Promise<string | null>
  openFolder: () => Promise<string | null>
  saveFile: (defaultPath?: string) => Promise<string | null>
}

export const useFileDialogStore = create<FileDialogState>(() => ({
  openFile: async (filters?: FileFilter[]) => {
    try {
      const result = await invoke<string | null>('open_file_dialog', {
        title: 'Select File',
        filters,
      })
      return result
    } catch (error) {
      console.error('Failed to open file dialog:', error)
      return null
    }
  },

  openFolder: async () => {
    try {
      const result = await invoke<string | null>('open_folder_dialog', {
        title: 'Select Folder',
      })
      return result
    } catch (error) {
      console.error('Failed to open folder dialog:', error)
      return null
    }
  },

  saveFile: async (defaultPath?: string) => {
    try {
      const result = await invoke<string | null>('save_file_dialog', {
        title: 'Save File',
        defaultPath,
      })
      return result
    } catch (error) {
      console.error('Failed to open save dialog:', error)
      return null
    }
  },
}))
