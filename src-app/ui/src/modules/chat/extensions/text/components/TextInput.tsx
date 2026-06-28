import { useEffect, useRef } from 'react'
import { Textarea } from '@/components/ui'
import { Stores } from '@/core/stores'

/**
 * TextInput Component
 * Self-contained text input for chat messages
 *
 * Features:
 * - Uses an uncontrolled ref for imperative get/set/clear access
 * - Registers getter/clearer functions with TextStore for external access
 * - Handles keyboard shortcuts (Enter to send, Shift+Enter for new line)
 * - Ref stays local (not frozen by immer), functions provide external access
 */
export function TextInput() {
  const ref = useRef<HTMLTextAreaElement>(null)
  const { sending } = Stores.Chat
  const {setGetMessage, setSetMessage, setClearMessage} = Stores.Chat.TextStore

  // Register getter/setter/clearer functions with TextStore on mount
  useEffect(() => {
    setGetMessage(() => ref.current?.value ?? '')
    setSetMessage((text: string) => { if (ref.current) ref.current.value = text })
    setClearMessage(() => { if (ref.current) ref.current.value = '' })
  }, [setGetMessage, setSetMessage, setClearMessage])

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
        onKeyDown={handleKeyDown}
        placeholder="Type your message..."
        autoSize={{ minRows: 2, maxRows: 8 }}
        disabled={sending}
        defaultValue=""
        className="resize-none !border-none focus:!border-none focus:!outline-none focus:!shadow-none !pt-1 bg-transparent text-base"
      />
    </div>
  )
}
