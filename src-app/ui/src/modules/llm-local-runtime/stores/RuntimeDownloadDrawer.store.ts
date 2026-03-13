import { create } from 'zustand'
import type { RuntimeEngine } from '../types'

interface RuntimeDownloadDrawerState {
  open: boolean
  engine: RuntimeEngine | null

  openDrawer: (engine: RuntimeEngine) => void
  closeDrawer: () => void
}

export const useRuntimeDownloadDrawerStore = create<RuntimeDownloadDrawerState>((set) => ({
  open: false,
  engine: null,

  openDrawer: (engine) => set({ open: true, engine }),
  closeDrawer: () => set({ open: false, engine: null })
}))
