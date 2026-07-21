import type { InlineFileViewState } from '@/modules/chat/core/stores/messageViewState.helpers'
import { DEFAULT_INLINE_FILE_STATE } from '@/modules/chat/core/stores/messageViewState.helpers'
import type { MessageViewStateSet } from '../state'

/** Ensure a file entry exists (seeded from defaults) before mutating it. */
export default (_set: MessageViewStateSet) => {
  return (
    files: Record<string, InlineFileViewState>,
    key: string,
  ): InlineFileViewState => {
    if (!files[key]) files[key] = { ...DEFAULT_INLINE_FILE_STATE }
    return files[key]
  }
}
