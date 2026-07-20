import type { ChatSet, ChatInitialState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, _getRaw: () => ChatInitialState) => {
  return async (
      runtime: import('@/modules/chat/core/extensions/types').ExtensionLifecycle | null,
    ) => {
      set({ extensionRuntime: runtime })
    }
}
