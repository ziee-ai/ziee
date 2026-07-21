import type { ConversationSummarizationSet } from '../state'

export default (set: ConversationSummarizationSet) =>
  async () => {
    set(s => {
      s.current = null
      s.requestedConversationId = null
      s.loading = false
      s.error = null
    })
  }
