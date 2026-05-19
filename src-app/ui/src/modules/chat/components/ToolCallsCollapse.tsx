import { useState, useEffect, useRef } from 'react'
import { Collapse } from 'antd'
import { ToolOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { ContentRenderer } from '@/modules/chat/components/ContentRenderer'
import type { MessageContent, MessageContentDataToolUse } from '@/api-client/types'

interface ToolCallsCollapseProps {
  toolGroupContents: MessageContent[]
  messageId: string
}

function buildTitle(toolUseContents: MessageContent[]): string {
  const names = toolUseContents
    .map(c => (c.content as MessageContentDataToolUse).name)
    .filter(Boolean)
  const unique = [...new Set(names)]
  if (unique.length === 0) return 'Tool calls'
  if (unique.length === 1) return `Used ${unique[0]}`
  if (unique.length === 2) return `Used ${unique[0]} and ${unique[1]}`
  return `Used ${unique[0]}, ${unique[1]}, and ${unique.length - 2} more`
}

export function ToolCallsCollapse({ toolGroupContents, messageId }: ToolCallsCollapseProps) {
  const { isStreaming, streamingMessage } = Stores.Chat
  const { toolCalls } = Stores.Chat.McpStore

  const toolUseContents = toolGroupContents.filter(c => c.content_type === 'tool_use')

  const isThisMessageStreaming = isStreaming && streamingMessage?.id === messageId
  const hasPendingApproval = toolUseContents.some(c => {
    const toolUse = c.content as MessageContentDataToolUse
    return toolCalls.get(toolUse.id)?.status === 'pending_approval'
  })
  const shouldBeOpen = isThisMessageStreaming || hasPendingApproval

  const userHasInteracted = useRef(false)
  const [activeKey, setActiveKey] = useState<string[]>(
    shouldBeOpen ? ['tool-calls'] : []
  )

  // React to streaming/approval state: auto-open while active, auto-close when done
  useEffect(() => {
    if (shouldBeOpen) {
      setActiveKey(['tool-calls'])
      userHasInteracted.current = false
    } else if (!userHasInteracted.current) {
      setActiveKey([])
    }
  }, [shouldBeOpen])

  const items = [
    {
      key: 'tool-calls',
      label: (
        <span className="flex items-center gap-2 text-sm">
          <ToolOutlined />
          {buildTitle(toolUseContents)}
          <span className="text-xs opacity-50">({toolUseContents.length})</span>
        </span>
      ),
      children: (
        <div className="flex flex-col gap-2">
          {toolGroupContents.map((content, index) => (
            <ContentRenderer key={content.id || index} content={content} isUser={false} />
          ))}
        </div>
      ),
    },
  ]

  return (
    <Collapse
      items={items}
      activeKey={activeKey}
      onChange={keys => {
        userHasInteracted.current = true
        setActiveKey(keys as string[])
      }}
      size="small"
    />
  )
}
