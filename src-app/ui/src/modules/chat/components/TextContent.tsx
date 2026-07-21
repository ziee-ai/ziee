import { memo } from 'react'
import { Streamdown } from '@/modules/chat/core/utils/LazyStreamdown'
import type { MessageContent } from '@/api-client/types'
import { useStreamdownComponents } from '@/modules/chat/core/utils/useStreamdownComponents'
import { StreamdownErrorBoundary } from '@/modules/chat/core/utils/StreamdownErrorBoundary'
import { citationTokenize } from '@/modules/chat/core/utils/citationTokenize'
import { streamdownUrlTransform } from '@/modules/chat/core/utils/streamdownUrlTransform'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

interface TextContentProps {
  content: MessageContent
  isUser: boolean
}

export const TextContent = memo(function TextContent({
  content,
  isUser,
}: TextContentProps) {
  const textData = content.content as { text?: string }
  const { isStreaming } = Chat
  const components = useStreamdownComponents(content.id)

  if (!textData.text) {
    return null
  }

  // User messages: plain text (no markdown)
  if (isUser) {
    return <div style={{ whiteSpace: 'pre-wrap' }}>{textData.text}</div>
  }

  // Assistant messages: render markdown using Streamdown
  return (
    <div className="w-full overflow-x-auto pt-2">
      <StreamdownErrorBoundary fallbackText={textData.text}>
        <Streamdown
          variant="chat"
          isAnimating={isStreaming}
          components={components}
          urlTransform={streamdownUrlTransform}
        >
          {citationTokenize(textData.text)}
        </Streamdown>
      </StreamdownErrorBoundary>
    </div>
  )
})
