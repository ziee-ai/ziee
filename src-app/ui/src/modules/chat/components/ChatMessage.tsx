import { Fragment, memo, useMemo, useRef, type ReactNode } from 'react'
import { ScrollArea } from '@/components/ui'
import { cn } from '@/lib/utils'
import type { MessageWithContent } from '@/api-client/types'
import { ExtensionSlot, chatExtensionRegistry } from '@/modules/chat/core/extensions'
import { ContentRenderer } from '@/modules/chat/components/ContentRenderer'
import { MessageContext } from '@/modules/chat/core/MessageContext'
import { BranchNavigator } from '@/modules/chat/components/BranchNavigator'
import { MessageActions } from '@/modules/chat/components/MessageActions'
import { CollapsibleBlock } from '@/modules/chat/components/CollapsibleBlock'
import { shouldOfferCollapse } from '@/modules/chat/components/collapsible'
import { messageText } from '@/modules/chat/components/findMatches'
import { useConversationFind } from '@/modules/chat/components/ConversationFindContext'

export const ChatMessage = memo(function ChatMessage({
  message,
  isStreaming = false,
}: {
  message: MessageWithContent
  /** True only for the message currently streaming — it is never collapsed. */
  isStreaming?: boolean
}) {
  const isUser = message.role === 'user'
  const { activeMatchId } = useConversationFind()
  const isActiveMatch = activeMatchId === message.id

  // Once a message has streamed in THIS mount, never retroactively clamp it:
  // snapping a long answer the user is reading from full height to a 384px
  // clamp the instant streaming ends is a jarring reflow (DEC-6 exempts only
  // the in-flight message; this extends that to the just-finished one). The ref
  // survives the isStreaming true→false transition; a fresh mount (reload) has
  // it false, so history still clamps.
  const wasStreamingRef = useRef(false)
  if (isStreaming) wasStreamingRef.current = true

  // Memoized so the find-highlight re-render (every ChatMessage consumes the
  // find context, so an active-match change re-renders them all) doesn't
  // recompute the message text + collapse decision each time — only when its
  // inputs change. The ACTIVE find match is never clamped, so the matched text
  // can't hide below the fold when find scrolls to it (isActiveMatch in deps
  // triggers recompute; reading the ref is safe because isStreaming is a dep).
  const offerCollapse = useMemo(() => {
    // Short-circuit BEFORE the O(n) messageText concat: a streaming, just-
    // streamed, or active-match message is never clamped, so don't rebuild the
    // full text on every streaming token (that would be O(n^2) over a stream).
    if (isStreaming || wasStreamingRef.current || isActiveMatch) return false
    return shouldOfferCollapse({
      length: messageText(message).length,
      isStreaming: false,
    })
  }, [message, isStreaming, isActiveMatch])

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
      className={cn(
        // Role is encoded in the geometry of THIS role-tagged element, not just
        // a nested bubble: user messages shrink-to-content and pin to the right
        // (self-end + w-fit, capped so they never span full width and read as
        // centered); assistant messages stay flush-left and full-width. This is
        // what lets a reader — and the C7 role-signature check — tell them apart.
        'flex flex-col overflow-visible group scroll-mt-24',
        isUser ? 'items-end self-end w-fit max-w-[85%]' : 'items-start w-full',
        // Transient highlight for the active in-conversation find match (ITEM-1).
        isActiveMatch && 'rounded-lg ring-2 ring-primary ring-offset-2 ring-offset-background transition-shadow',
      )}
      data-testid="chat-message"
      data-role={message.role}
      data-message-id={message.id}
      data-find-active={isActiveMatch ? '' : undefined}
    >
      {/* User attachments: a single horizontal row above the bubble that
          x-scrolls (via the app's overlay ScrollArea) when it overflows.
          max-w-full (not w-full): the outer container is a flex column with
          items-end, so a content-width strip shrinks-to-fit and pins to the
          RIGHT edge — matching the right-aligned user bubble — instead of
          spanning full width and stranding the files on the left. The cap keeps
          it inside the bubble's max width, so a long list still x-scrolls. */}
      {attachmentBlocks.length > 0 && (
        <ScrollArea
          axis="x"
          className="max-w-full mb-2"
          data-testid="message-attachments"
        >
          {/* ml-auto: right-align the file row inside the scroll viewport so a
              short list packs against the bubble's right edge (matching the
              right-aligned user message); a no-op once the row overflows (it just
              scrolls). */}
          <div className="flex gap-2 w-max py-0.5 ms-auto">
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
          className={cn(
            'rounded-lg relative flex flex-col',
            // User: a subtle token-driven tint (reads as a "bubble" in both
            // themes) hugging its content. Assistant: flush, borderless,
            // full-width — no avatar, no card.
            isUser
              ? 'bg-primary/10 w-fit max-w-full px-3 py-2'
              : 'bg-transparent w-full p-0',
          )}
        >
          <div
            className={
              // px-0.5: the bubble inner is overflow-x-hidden (it clips wide code
              // blocks), and the assistant body has no padding, so a full-width
              // child Card (MCP tool-call / tool-group card) sits FLUSH against the
              // clip edge — its border + rounded corners get shaved on the left/
              // right. A 2px inset gives every card edge room to render fully while
              // the overflow clip still contains wide content.
              'flex flex-1 w-full overflow-x-hidden flex-col px-0.5'
            }
          >
            {offerCollapse ? (
              <CollapsibleBlock
                className="w-full"
                data-testid="chat-message-collapsible"
              >
                <div className={'w-full flex flex-col gap-2'}>{bubbleNodes}</div>
              </CollapsibleBlock>
            ) : (
              <div className={'w-full flex flex-col gap-2'}>{bubbleNodes}</div>
            )}
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
