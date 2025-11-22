import { Input, Button } from 'antd'
import { SendOutlined } from '@ant-design/icons'
import { useState, type KeyboardEvent } from 'react'

interface ChatInputProps {
  onSend: (content: string) => void
  disabled?: boolean
  loading?: boolean
  placeholder?: string
}

export function ChatInput({
  onSend,
  disabled = false,
  loading = false,
  placeholder = 'Message...'
}: ChatInputProps) {
  const [value, setValue] = useState('')

  const handleSend = () => {
    const trimmed = value.trim()
    if (trimmed && !disabled && !loading) {
      onSend(trimmed)
      setValue('')
    }
  }

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Send on Enter (without Shift)
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSend()
    }
  }

  return (
    <div className="flex gap-2 items-end">
      <Input.TextArea
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        autoSize={{ minRows: 1, maxRows: 6 }}
        disabled={disabled}
        className="flex-1"
      />
      <Button
        type="primary"
        icon={<SendOutlined />}
        onClick={handleSend}
        disabled={disabled || !value.trim()}
        loading={loading}
        size="large"
      >
        Send
      </Button>
    </div>
  )
}
