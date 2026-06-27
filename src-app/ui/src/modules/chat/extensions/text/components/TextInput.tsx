import { useEffect } from 'react'
import { Input, Form } from 'antd'
import { Stores } from '@/core/stores'

const { TextArea } = Input

/**
 * TextInput Component
 * Self-contained text input for chat messages
 *
 * Features:
 * - Creates its own Form instance (no parent dependency)
 * - Registers getter/clearer functions with TextStore for external access
 * - Handles keyboard shortcuts (Enter to send, Shift+Enter for new line)
 * - Form stays local (not frozen by immer), functions provide external access
 */
export function TextInput() {
  const [form] = Form.useForm()
  const { sending } = Stores.Chat
  const {setGetMessage, setSetMessage, setClearMessage} = Stores.Chat.TextStore

  // Register getter/setter/clearer functions with TextStore on mount
  useEffect(() => {
    setGetMessage(() => form.getFieldValue('message') || '')
    setSetMessage((text: string) => form.setFieldValue('message', text))
    setClearMessage(() => form.setFieldValue('message', ''))
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
      <Form name="chat-text-input" form={form} initialValues={{ message: '' }}>
        <Form.Item name="message" className="mb-0" noStyle>
          <TextArea
            aria-label="Message"
            onKeyDown={handleKeyDown}
            placeholder="Type your message..."
            autoSize={{ minRows: 2, maxRows: 8 }}
            disabled={sending}
            className="resize-none !border-none focus:!border-none focus:!outline-none focus:!shadow-none !pt-1"
            style={{ backgroundColor: 'transparent', fontSize: 16 }}
          />
        </Form.Item>
      </Form>
    </div>
  )
}
