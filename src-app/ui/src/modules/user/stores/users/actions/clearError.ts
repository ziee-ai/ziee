import type { UsersSet, UsersGet } from '../state'

// Fires reactively (an error effect dismissing a surfaced message), so it has no
// hover trigger — the store's `init` warms it via `.preload()` instead, which is
// how the prefetch gate is satisfied for programmatic (non-click) actions.
export default (set: UsersSet, _get: UsersGet) => async (): Promise<void> => {
  set({ error: null })
}
