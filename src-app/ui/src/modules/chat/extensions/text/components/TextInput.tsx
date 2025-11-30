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

  // Register getter/clearer functions with TextStore on mount
  useEffect(() => {
    // Access store only inside effect to avoid hook timing issues
    const textStore = Stores.Chat.__state.TextStore
    if (textStore) {
      // Register getter function (captures form via closure)
      textStore.setGetMessage(() => form.getFieldValue('message') || '')

      // Register clearer function (captures form via closure)
      textStore.setClearMessage(() => form.setFieldValue('message', ''))
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

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
            onKeyDown={handleKeyDown}
            placeholder="Type your message..."
            autoSize={{ minRows: 1, maxRows: 6 }}
            disabled={sending}
            className="resize-none !border-none focus:!border-none focus:!outline-none focus:!shadow-none !pt-1"
            style={{ backgroundColor: 'transparent' }}
          />
        </Form.Item>
      </Form>
    </div>
  )
}
