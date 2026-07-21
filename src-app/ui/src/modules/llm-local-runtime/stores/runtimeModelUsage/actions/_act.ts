import type { RuntimeModelUsageSet } from '../state'

export default (set: RuntimeModelUsageSet) =>
  async <T>(modelId: string, fn: () => Promise<T>): Promise<T> => {
    set(state => ({
      acting: new Map(state.acting).set(modelId, true),
      error: null,
    }))
    try {
      return await fn()
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Action failed' })
      throw error
    } finally {
      set(state => {
        const acting = new Map(state.acting)
        acting.delete(modelId)
        return { acting }
      })
    }
  }
