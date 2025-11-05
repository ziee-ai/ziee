/**
 * Window Store
 *
 * Manages window state and Tauri window commands
 */

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

interface WindowState {
  isMaximized: boolean
  isFullscreen: boolean

  // Actions
  minimize: () => Promise<void>
  maximize: () => Promise<void>
  unmaximize: () => Promise<void>
  close: () => Promise<void>
  toggleFullscreen: () => Promise<void>
  checkIsMaximized: () => Promise<void>
}

export const useWindowStore = create<WindowState>((set, get) => ({
  isMaximized: false,
  isFullscreen: false,

  minimize: async () => {
    try {
      await invoke('minimize_window')
    } catch (error) {
      console.error('Failed to minimize window:', error)
    }
  },

  maximize: async () => {
    try {
      await invoke('maximize_window')
      set({ isMaximized: true })
    } catch (error) {
      console.error('Failed to maximize window:', error)
    }
  },

  unmaximize: async () => {
    try {
      await invoke('unmaximize_window')
      set({ isMaximized: false })
    } catch (error) {
      console.error('Failed to unmaximize window:', error)
    }
  },

  close: async () => {
    try {
      await invoke('close_window')
    } catch (error) {
      console.error('Failed to close window:', error)
    }
  },

  toggleFullscreen: async () => {
    try {
      await invoke('toggle_fullscreen')
      set({ isFullscreen: !get().isFullscreen })
    } catch (error) {
      console.error('Failed to toggle fullscreen:', error)
    }
  },

  checkIsMaximized: async () => {
    try {
      const maximized = await invoke<boolean>('is_window_maximized')
      set({ isMaximized: maximized })
    } catch (error) {
      console.error('Failed to check maximize state:', error)
    }
  },
}))
