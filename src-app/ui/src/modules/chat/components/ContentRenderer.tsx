import { memo } from 'react'
import type { MessageContentItem } from '@/api-client/types'
import { TextContent } from './TextContent'

interface ContentRendererProps {
  content: MessageContentItem
  isUser: boolean
}

export const ContentRenderer = memo(function ContentRenderer({
  content,
  isUser,
}: ContentRendererProps) {
  switch (content.content_type) {
    case 'text':
      return <TextContent content={content} isUser={isUser} />

    // Add other content types as needed:
    // case 'tool_call':
    // case 'tool_result':
    // case 'file_attachment':
    // case 'error':

    default:
      return (
        <div className="text-gray-500">
          Unknown content type: {content.content_type}
        </div>
      )
  }
})
