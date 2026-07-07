import { memo } from 'react'
import { Streamdown } from 'streamdown'
import type { MessageContent } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { useStreamdownComponents } from '@/modules/chat/core/utils/useStreamdownComponents'
import { StreamdownErrorBoundary } from '@/modules/chat/core/utils/StreamdownErrorBoundary'
import { streamdownUrlTransform } from '@/modules/chat/core/utils/streamdownUrlTransform'
import { mermaidRenderers } from '@/modules/chat/core/utils/mermaidRenderers'

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

  // Both user and assistant text render as markdown (code blocks, tables, etc.).
  // Only the assistant's LIVE stream animates; a user message is never streaming.
  return (
    <div className="w-full overflow-x-auto pt-2">
      <StreamdownErrorBoundary fallbackText={textData.text}>
        <Streamdown
          isAnimating={!isUser && isStreaming}
          shikiTheme={['github-light', 'github-dark']}
          components={components}
          plugins={mermaidRenderers}
          urlTransform={streamdownUrlTransform}
        >
          {textData.text}
        </Streamdown>
      </StreamdownErrorBoundary>
    </div>
  )
})
