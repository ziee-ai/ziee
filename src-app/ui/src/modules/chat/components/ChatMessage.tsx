import { Fragment, memo, type ReactNode } from 'react'
import { Avatar, ScrollArea } from '@/components/ui'
import type { MessageWithContent } from '@/api-client/types'
import { ExtensionSlot, chatExtensionRegistry } from '@/modules/chat/core/extensions'
import { ContentRenderer } from '@/modules/chat/components/ContentRenderer'
import { MessageContext } from '@/modules/chat/core/MessageContext'
import { BranchNavigator } from '@/modules/chat/components/BranchNavigator'
import { MessageActions } from '@/modules/chat/components/MessageActions'

export const ChatMessage = memo(function ChatMessage({
  message,
}: {
  message: MessageWithContent
}) {
  const isUser = message.role === 'user'

  // Check if message has any content to render
  if (!message.contents || message.contents.length === 0) {
    return null
  }

  // Render blocks in their authoritative backend order. Sort by
  // `sequence_order` (a copy — never mutate the store's array during
  // render), NOT `created_at`: blocks written in one DB transaction can
  // share a timestamp, and streaming-injected blocks carry monotonic
  // sequence_order. This keeps tool_use → tool_result(files) → text in
  // the right places.
  const sortedContents = [...message.contents].sort(
    (a, b) => a.sequence_order - b.sequence_order,
  )

  // For user messages, file attachments lift OUT of the text bubble and render
  // as a single horizontal row ABOVE it (outside the bordered box) that
  // x-scrolls when it overflows, instead of wrapping or stacking vertically.
  // Assistant messages keep every block in the body (which has no bubble border
  // anyway), so they're unchanged.
  const attachmentBlocks = isUser
    ? sortedContents.filter(c => c.content_type === 'file_attachment')
    : []
  const bubbleBlocks = isUser
    ? sortedContents.filter(c => c.content_type !== 'file_attachment')
    : sortedContents

  // Render blocks with a run-loop (not a plain map): a renderer that claims a
  // block can consume the blocks that follow it (via its static `contentSpan`),
  // so e.g. the MCP extension can fold a consecutive tool_use/tool_result run
  // into one "N tools called" group. `renderContent` reports how many blocks it
  // took; we advance past them. A block no extension claims falls back to the
  // built-in ContentRenderer (consumes 1).
  const bubbleNodes: ReactNode[] = []
  for (let i = 0; i < bubbleBlocks.length; ) {
    const block = bubbleBlocks[i]
    const key = block.id || `blk-${i}`
    const res = chatExtensionRegistry.renderContent({
      content: block,
      isUser,
      blocks: bubbleBlocks,
      index: i,
    })
    if (res) {
      bubbleNodes.push(<Fragment key={key}>{res.node}</Fragment>)
      i += res.consumed
    } else {
      bubbleNodes.push(
        <ContentRenderer key={key} content={block} isUser={isUser} />,
      )
      i += 1
    }
  }

  return (
    <div
      className={'w-full flex flex-col overflow-visible group'}
      data-testid="chat-message"
      data-role={message.role}
      data-message-id={message.id}
    >
      {/* User attachments: a single horizontal row above the bubble that
          x-scrolls (via the app's overlay ScrollArea) when it overflows. */}
      {attachmentBlocks.length > 0 && (
        <ScrollArea
          axis="x"
          className="w-full mb-2"
          data-testid="message-attachments"
        >
          <div className="flex gap-2 w-max py-0.5">
            {attachmentBlocks.map((content, index) => (
              <ContentRenderer
                key={`${content.id || `att-${index}`}`}
                content={content}
                isUser={isUser}
              />
            ))}
          </div>
        </ScrollArea>
      )}

      {/* Text bubble — only when there is non-attachment content. A files-only
          message has no text block (the text extension emits none for empty
          input), so bubbleBlocks is empty and no empty bubble renders. */}
      {bubbleBlocks.length > 0 && (
        <div
          key={message.id}
          className={`flex gap-2 rounded-lg relative min-w-36 flex-col ${
            isUser ? 'bg-card border-border w-fit p-2' : 'bg-transparent w-full p-0'
          }`}
        >
          <div className={'flex items-start gap-2 w-full relative'}>
            <div className={`flex ${!isUser ? 'hidden' : ''}`}>
              <Avatar src={undefined} className="h-8 w-8" />
            </div>

            {/* Message content */}
            <div
              className={`${isUser ? '!pt-0.5' : ''} flex flex-1 -mt-[2px] w-full overflow-x-hidden flex-col`}
            >
              <div className={'w-full flex flex-col gap-2'}>{bubbleNodes}</div>
            </div>
          </div>
        </div>
      )}

      {/* Core components + extension slots rendered outside the bubble */}
      <MessageContext.Provider value={message}>
        {/* Generic below-the-bubble extension point. Tool-returned files now
            render inline at their tool_result block (see the file extension's
            `tool_result` content renderer), so nothing registers here today. */}
        <ExtensionSlot name="message_footer" />
        <div className="flex flex-row items-center gap-1 mt-1">
          <BranchNavigator />
          <MessageActions />
          {/* Extensions can register additional message actions here */}
          <ExtensionSlot name="message_actions" />
        </div>
      </MessageContext.Provider>
    </div>
  )
})
