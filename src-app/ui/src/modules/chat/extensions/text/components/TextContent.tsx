import { memo } from 'react'
import { cn } from '@/lib/utils'
import { Streamdown } from '@/modules/chat/core/utils/LazyStreamdown'
import type { MessageContent } from '@/api-client/types'
import { Stores } from '@ziee/framework/stores'
import { useStreamdownComponents } from '@/modules/chat/core/utils/useStreamdownComponents'
import { StreamdownErrorBoundary } from '@/modules/chat/core/utils/StreamdownErrorBoundary'
import { streamdownUrlTransform } from '@/modules/chat/core/utils/streamdownUrlTransform'
import { chatMarkdownPlugins } from '@/modules/chat/core/utils/chatMarkdownPlugins'
import { preprocessMarkdown } from '@/components/common/markdownPreprocess'
import { citationTokenize } from '@/modules/chat/core/utils/citationTokenize'

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
    <div className={cn(
      'w-full overflow-x-auto',
      // pt-2 gives assistant blocks a little top breathing room when stacked.
      // A user message is a single centered bubble (px-3 py-2) — the extra top
      // padding would push its text off-center (16px top vs 8px bottom), so it's
      // assistant-only.
      !isUser && 'pt-2',
    )}>
      <StreamdownErrorBoundary fallbackText={textData.text}>
        <Streamdown
          isAnimating={!isUser && isStreaming}
          shikiTheme={['github-light-high-contrast', 'github-dark-high-contrast']}
          plugins={chatMarkdownPlugins}
          components={components}
          urlTransform={streamdownUrlTransform}
        >
          {preprocessMarkdown(
            // Assistant-only: rewrite bare `[n]` KB citations into chip links.
            isUser ? textData.text : citationTokenize(textData.text),
          )}
        </Streamdown>
      </StreamdownErrorBoundary>
    </div>
  )
})
