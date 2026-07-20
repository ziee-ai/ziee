import type { WebSearchAdminState } from '../state'

/** Composite: fan out to loadSettings + loadProviders (sibling dispatchers,
 *  reached via the runtime state). Its own lazy chunk like every action. */
export default (_set: unknown, getRaw: () => WebSearchAdminState) =>
  async (): Promise<void> => {
    const s = getRaw() as WebSearchAdminState & {
      loadSettings: () => Promise<void>
      loadProviders: () => Promise<void>
    }
    await Promise.all([s.loadSettings(), s.loadProviders()])
  }
