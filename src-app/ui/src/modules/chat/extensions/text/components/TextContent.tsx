import { memo } from 'react'
import { Streamdown } from 'streamdown'
import type { MessageContent } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface TextContentProps {
  content: MessageContent
  isUser: boolean
}

export const TextContent = memo(function TextContent({
  content,
  isUser,
}: TextContentProps) {
  const textData = content.content as { text?: string }
  const { isStreaming } = Stores.Chat

  if (!textData.text) {
    return null
  }

  // User messages: plain text (no markdown)
  if (isUser) {
    return <div style={{ whiteSpace: 'pre-wrap' }}>{textData.text}</div>
  }

  // Assistant messages: streaming markdown
  return (
    <div className="w-full overflow-hidden pt-2 pl-2">
      <Streamdown
        isAnimating={isStreaming}
        shikiTheme={['github-light', 'github-dark']}
      >
        {textData.text}
      </Streamdown>
    </div>
  )
})
