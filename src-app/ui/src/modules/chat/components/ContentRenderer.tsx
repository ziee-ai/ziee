import { memo } from 'react'
import type { MessageContent } from '@/api-client/types'
import { chatExtensionRegistry } from '@/modules/chat/core/extensions'
import { TextContent } from '@/modules/chat/components/TextContent'

interface ContentRendererProps {
  content: MessageContent
  isUser: boolean
}

export const ContentRenderer = memo(function ContentRenderer({
  content,
  isUser,
}: ContentRendererProps) {
  // Try extension rendering first
  const extensionRenderer = chatExtensionRegistry.renderContent({
    content,
    isUser,
  })

  if (extensionRenderer !== null) {
    return <>{extensionRenderer}</>
  }

  // Fall back to built-in renderers
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
