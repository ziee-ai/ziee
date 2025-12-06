import { memo } from 'react'
import type { MessageContent } from '@/api-client/types'

interface ThinkingContentProps {
  content: MessageContent
  isUser: boolean
}

export const ThinkingContent = memo(function ThinkingContent({
  content,
}: ThinkingContentProps) {
  const thinkingData = content.content as { thinking?: string }

  if (!thinkingData.thinking) {
    return null
  }

  // Thinking content is always from the assistant
  return (
    <div className="w-full overflow-hidden pt-2 pl-2">
      <div className="border-l-4">
        <div className="text-sm font-semibold mb-1">
          Thinking...
        </div>
        <div className="text-sm" style={{ whiteSpace: 'pre-wrap' }}>
          {thinkingData.thinking.trim()}
        </div>
      </div>
    </div>
  )
})
