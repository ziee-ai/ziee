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
      <div className="border-l-4 border-blue-500 pl-4 py-2 bg-blue-50 dark:bg-blue-900/20">
        <div className="text-sm font-semibold text-blue-700 dark:text-blue-300 mb-1">
          Thinking...
        </div>
        <div className="text-sm text-gray-700 dark:text-gray-300" style={{ whiteSpace: 'pre-wrap' }}>
          {thinkingData.thinking.trim()}
        </div>
      </div>
    </div>
  )
})
