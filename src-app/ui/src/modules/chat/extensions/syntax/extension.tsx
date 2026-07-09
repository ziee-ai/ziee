import { useState } from 'react'
import { Button, message } from '@/components/ui'
import { Check, Copy as CopyIcon } from 'lucide-react'
import {
  createExtension,

  type ChatExtension,
  type ContentRendererProps,
} from '@/modules/chat/core/extensions'

/**
 * Code block component with copy functionality
 */
function CodeBlock({ code, language }: { code: string; language?: string }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      message.success('Code copied to clipboard')
      setTimeout(() => setCopied(false), 2000)
    } catch (_error) {
      message.error('Failed to copy code')
    }
  }

  return (
    <div className="relative group mb-2">
      {/* Language label and copy button */}
      <div className="flex items-center justify-between px-3 py-1 text-xs rounded-t">
        <span>{language || 'code'}</span>
        <Button
          data-testid="chat-code-copy-btn"
          size="default"
          variant="ghost"
          icon={copied ? <Check /> : <CopyIcon />}
          onClick={handleCopy}
          className="opacity-0 group-hover:opacity-100 hover-none:opacity-100 transition-opacity"
        >
          {copied ? 'Copied' : 'Copy'}
        </Button>
      </div>

      {/* Code content */}
      <pre className="p-3 rounded-b overflow-x-auto">
        <code className={language ? `language-${language}` : ''}>{code}</code>
      </pre>
    </div>
  )
}

/**
 * Parse text content and extract code blocks
 * Returns array of text and code segments
 */
function parseMarkdownCodeBlocks(text: string): Array<{
  type: 'text' | 'code'
  content: string
  language?: string
}> {
  const segments: Array<{
    type: 'text' | 'code'
    content: string
    language?: string
  }> = []

  // Match code blocks: ```language\ncode\n```
  const codeBlockRegex = /```(\w+)?\n([\s\S]*?)```/g
  let lastIndex = 0
  let match: RegExpExecArray | null

  while ((match = codeBlockRegex.exec(text)) !== null) {
    // Add text before code block
    if (match.index > lastIndex) {
      segments.push({
        type: 'text',
        content: text.substring(lastIndex, match.index),
      })
    }

    // Add code block
    segments.push({
      type: 'code',
      content: match[2].trim(),
      language: match[1] || undefined,
    })

    lastIndex = match.index + match[0].length
  }

  // Add remaining text
  if (lastIndex < text.length) {
    segments.push({
      type: 'text',
      content: text.substring(lastIndex),
    })
  }

  return segments.length > 0 ? segments : [{ type: 'text', content: text }]
}

/**
 * Enhanced text renderer with markdown code block support
 */
function EnhancedTextContent({
  text,
  isUser,
}: {
  text: string
  isUser: boolean
}) {
  // User messages don't need code block parsing
  if (isUser) {
    return <div style={{ whiteSpace: 'pre-wrap' }}>{text}</div>
  }

  // Parse and render assistant messages with code blocks
  const segments = parseMarkdownCodeBlocks(text)

  return (
    <div className="w-full overflow-hidden pt-2 pl-2">
      {segments.map((segment, index) => {
        if (segment.type === 'code') {
          return (
            <CodeBlock
              key={index}
              code={segment.content}
              language={segment.language}
            />
          )
        }

        return (
          <div key={index} style={{ whiteSpace: 'pre-wrap' }}>
            {segment.content.trim()}
          </div>
        )
      })}
    </div>
  )
}

/**
 * Text content renderer component with syntax highlighting
 */
function TextContentRenderer({ content: data, isUser }: ContentRendererProps) {
  const textData = data.content as { text?: string }

  if (!textData.text) {
    return null
  }

  return <EnhancedTextContent text={textData.text} isUser={isUser} />
}

/**
 * Syntax Extension
 * Enhances text rendering with code syntax highlighting
 */
const syntaxExtension: ChatExtension = createExtension({
  name: 'syntax',
  description: 'Provides code syntax highlighting and markdown parsing',
  priority: 70, // Lower priority to allow other extensions to handle content first

  // No per-conversation state needed

  // Register content type components
  contentTypes: {
    text: TextContentRenderer,
  },
})

export default syntaxExtension
