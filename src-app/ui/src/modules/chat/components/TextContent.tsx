import { memo } from 'react'
import type { MessageContentItem } from '@/api-client/types'

interface TextContentProps {
  content: MessageContentItem
  isUser: boolean
}

export const TextContent = memo(function TextContent({
  content,
  isUser,
}: TextContentProps) {
  const textData = content.content as { text?: string }

  if (!textData.text) {
    return null
  }

  if (isUser) {
    return <div style={{ whiteSpace: 'pre-wrap' }}>{textData.text}</div>
  }

  // For assistant messages, render with pre-wrap for now
  // TODO: Add markdown renderer later
  return (
    <div className={'w-full overflow-hidden pt-2 pl-2'}>
      <div style={{ whiteSpace: 'pre-wrap' }}>{textData.text.trim()}</div>
    </div>
  )
})
