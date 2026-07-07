import { useEffect, useRef } from 'react'
import { Textarea } from '@/components/ui'
import { Stores } from '@/core/stores'
import {
  getDraft,
  setDraft,
  clearDraft,
  NEW_DRAFT_KEY,
} from '@/modules/chat/extensions/text/chatDrafts'

/**
 * TextInput Component
 * Self-contained text input for chat messages
 *
 * Features:
 * - Uses an uncontrolled ref for imperative get/set/clear access
 * - Registers getter/clearer functions with TextStore for external access
 * - Handles keyboard shortcuts (Enter to send, Shift+Enter for new line)
 * - Persists an unsent draft per conversation across navigation (ITEM-7)
 * - Ref stays local (not frozen by immer), functions provide external access
 */
export function TextInput() {
  const ref = useRef<HTMLTextAreaElement>(null)
  const { sending } = Stores.Chat
  const { setGetMessage, setSetMessage, setClearMessage, setClearDraft } =
    Stores.Chat.TextStore

  // The draft bucket for the composer's CURRENT conversation. `new` when there
  // is no conversation yet (new-chat page). Read reactively so it follows an
  // in-app A→B conversation switch. `editingMessage` gates persistence so an
  // edit/regenerate prefill (which calls TextStore.setText) is never captured
  // as, or clobbered by, a draft (DEC-7).
  const conversationId = Stores.Chat.conversation?.id
  const isEditing = Stores.Chat.editingMessage != null
  const draftKey = conversationId ?? NEW_DRAFT_KEY

  // Keep the latest key/editing flag in refs so the DOM-driven save handler and
  // the registered clearer read current values without re-subscribing.
  const draftKeyRef = useRef(draftKey)
  const isEditingRef = useRef(isEditing)
  draftKeyRef.current = draftKey
  isEditingRef.current = isEditing

  // Register getter/setter/clearer functions with TextStore on mount.
  useEffect(() => {
    setGetMessage(() => ref.current?.value ?? '')
    setSetMessage((text: string) => {
      if (ref.current) ref.current.value = text
    })
    setClearMessage(() => {
      if (ref.current) ref.current.value = ''
    })
    // On successful send, clear the persisted draft for the key that was active
    // when the user typed (and the `new` bucket — see clearDraft).
    setClearDraft(() => clearDraft(draftKeyRef.current))
  }, [setGetMessage, setSetMessage, setClearMessage, setClearDraft])

  // Restore the saved draft when the composer (re)binds to a conversation key.
  // Only when NOT editing and the textarea is empty, so we never overwrite an
  // edit/regenerate prefill or in-progress typing (DEC-7).
  useEffect(() => {
    if (isEditing) return
    const el = ref.current
    if (!el) return
    if (el.value.length > 0) return
    const saved = getDraft(draftKey)
    if (saved) el.value = saved
  }, [draftKey, isEditing])

  // Persist on input (debounced). Suppressed while editing so the edit buffer
  // isn't written over the real draft.
  const handleInput = () => {
    if (isEditingRef.current) return
    setDraft(draftKeyRef.current, ref.current?.value ?? '')
  }

  const handleKeyDown = async (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Submit on Enter (without Shift)
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()

      // Call sendMessage directly - model extension will provide model_id
      await Stores.Chat.sendMessage()
    }
  }

  return (
    <div className="w-full">
      <Textarea
        data-testid="chat-message-textarea"
        ref={ref}
        aria-label="Message"
        onKeyDown={handleKeyDown}
        onChange={handleInput}
        placeholder="Type your message..."
        autoSize={{ minRows: 2, maxRows: 8 }}
        disabled={sending}
        defaultValue=""
        className="resize-none !border-none bg-transparent dark:bg-transparent text-base !py-1 outline-none focus-visible:ring-0 focus-visible:border-transparent focus-visible:shadow-none"
      />
    </div>
  )
}
