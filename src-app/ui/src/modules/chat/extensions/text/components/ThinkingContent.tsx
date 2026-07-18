import { memo, useState } from 'react'
import { Brain, ChevronDown } from 'lucide-react'
import { Button, Card, Text } from '@ziee/kit'
import { cn } from '@/lib/utils'
import type { MessageContent } from '@/api-client/types'

interface ThinkingContentProps {
  content: MessageContent
  isUser: boolean
}

export const ThinkingContent = memo(function ThinkingContent({
  content,
}: ThinkingContentProps) {
  const thinkingData = content.content as { thinking?: string }
  const [isExpanded, setIsExpanded] = useState(false)

  const text = thinkingData.thinking?.trim()
  if (!text) {
    return null
  }

  // Thinking content is always from the assistant. Rendered as a collapsible
  // card that mirrors the MCP tool-call cards (Card size="sm", header row with
  // icon + label + chevron, tight when collapsed) — so reasoning folds away by
  // default and reads as the same class of "process" affordance as a tool call.
  return (
    <Card
      size="sm"
      className={cn('mb-2', !isExpanded && 'py-2.5')}
      data-testid="thinking-card"
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 min-w-0">
          <Brain className="size-4 text-muted-foreground shrink-0" />
          <Text strong className="truncate">
            Thinking
          </Text>
        </div>
        <Button
          size="icon"
          variant="ghost"
          tooltip={isExpanded ? 'Hide details' : 'Show details'}
          icon={<ChevronDown className={cn('transition-transform', isExpanded && 'rotate-180')} />}
          onClick={() => setIsExpanded(!isExpanded)}
          data-testid="thinking-details-btn"
        />
      </div>

      {isExpanded && (
        <div className="mt-2 text-sm text-muted-foreground whitespace-pre-wrap">
          {text}
        </div>
      )}
    </Card>
  )
})
