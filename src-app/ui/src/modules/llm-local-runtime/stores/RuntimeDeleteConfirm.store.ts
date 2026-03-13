import { create } from 'zustand'
import type { RuntimeVersionResponse } from '@/api-client/types'

interface RuntimeDeleteConfirmState {
  version: RuntimeVersionResponse | null

  setVersion: (version: RuntimeVersionResponse | null) => void
}

export const useRuntimeDeleteConfirmStore = create<RuntimeDeleteConfirmState>((set) => ({
  version: null,
  setVersion: (version) => set({ version })
}))
