/**
 * Window Store — window state + Tauri window commands.
 */
import { invoke } from '@tauri-apps/api/core'
import { defineStore } from '@ziee/framework/store-kit'

export const Window = defineStore('Window', {
  state: {
    isMaximized: false,
    isFullscreen: false,
  },
  actions: (set, get) => ({
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
  }),
})

export const useWindowStore = Window.store
