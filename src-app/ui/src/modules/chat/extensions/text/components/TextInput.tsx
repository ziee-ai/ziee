import { useEffect, useRef } from 'react'
import { Textarea } from '@/components/ui'
import { Stores } from '@/core/stores'
import {
  getDraft,
  setDraft,
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
  const { setGetMessage, setSetMessage, setClearMessage } =
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
  }, [setGetMessage, setSetMessage, setClearMessage])

  // Restore the saved draft when the composer binds to a NEW conversation key.
  // ConversationPage/TextInput are reused (not remounted) across an in-app
  // A→B switch, so we must key restoration off draftKey CHANGES (tracked in a
  // ref) — restoring once per key REPLACES the textarea with the target key's
  // draft. This both (a) loads B's draft and (b) discards A's leftover text
  // (already persisted under A), preventing it from bleeding into B and being
  // re-saved under B's key. Suppressed while editing so an edit/regenerate
  // prefill is never clobbered (DEC-7); re-restoring within the same key is a
  // no-op so in-progress typing is never overwritten.
  const restoredKeyRef = useRef<string | null>(null)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    if (isEditing) {
      // Entering edit: the edit prefill (TextStore.setText) owns the textarea.
      // Forget the "already restored" latch so that when the edit ends —
      // cancelEdit clears the textarea WITHOUT changing draftKey — the effect
      // re-runs and re-restores the pre-edit draft instead of leaving it empty.
      restoredKeyRef.current = null
      return
    }
    if (restoredKeyRef.current === draftKey) return
    restoredKeyRef.current = draftKey
    el.value = getDraft(draftKey)
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
