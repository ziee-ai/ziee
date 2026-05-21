import { memo } from 'react'
import { Streamdown } from 'streamdown'
import type { MessageContent } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { useStreamdownComponents } from '@/modules/chat/core/utils/useStreamdownComponents'

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

  const components = useStreamdownComponents(content.id)

  if (!textData.text) {
    return null
  }

  // User messages: plain text (no markdown)
  if (isUser) {
    return <div style={{ whiteSpace: 'pre-wrap' }}>{textData.text}</div>
  }

  // Assistant messages: streaming markdown
  return (
    <div className="w-full overflow-x-auto pt-2">
      <Streamdown
        isAnimating={isStreaming}
        shikiTheme={['github-light', 'github-dark']}
        components={components}
      >
        {textData.text}
      </Streamdown>
    </div>
  )
})
