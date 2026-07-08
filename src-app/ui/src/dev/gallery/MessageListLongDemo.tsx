/**
 * message-scroll-stability ITEM-1 — a ~500-message MIXED-content conversation
 * driving the REAL virtualized `MessageList` inside a fixed-height scroll box,
 * so the scroll-stability e2e (TEST-6..10, TEST-12) can measure the virtualizer
 * on a surface that reproduces the jitter root cause (variable-height inline
 * content: long collapsible text, markdown tables, images, and inline file
 * previews). Backend-free: images are inline `data:` SVG URIs and inline file
 * previews are URL-based (no fetch), so the whole surface renders offline.
 *
 * The correction counter it exercises is exposed on `window.__MSGLIST_METRICS__`
 * by MessageList (DEV-only).
 */
import { useEffect, useRef, useState } from 'react'
import { Button } from '@/components/ui'
import {
  MessageList,
  type MessageListHandle,
} from '@/modules/chat/components/MessageList'
import { useChatStore } from '@/modules/chat/core/stores/Chat.store'
import { useMessageViewStateStore } from '@/modules/chat/core/stores/MessageViewState.store'
import type { MessageContent, MessageWithContent } from '@/api-client/types'

const NOW = new Date('2026-07-08T00:00:00Z').toISOString()

// A tiny inline PNG (1x1) so the image viewer claims it (image/png) and decodes
// instantly + offline — exercises the inline-image body inside the fixed-height
// box without a network image. (SVG mime is not claimed by the image viewer.)
const PNG_IMG =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M8AAAMBAQDJ/pLvAAAAAElFTkSuQmCC'
// A larger inline SVG used INSIDE markdown image messages (not a file preview).
const SVG_IMG =
  'data:image/svg+xml;utf8,' +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="320" height="180"><rect width="320" height="180" fill="%23888"/></svg>',
  )

const LONG_TEXT = (() => {
  const p =
    'This is a deliberately long assistant answer that exceeds the collapse threshold so the message offers a Show more / Show less toggle. '
  return p.repeat(14) // ~1900 chars → > COLLAPSE_CHAR_THRESHOLD (1200)
})()

const MD_TABLE = [
  '| Name | Qty | Note |',
  '| --- | --- | --- |',
  '| Banana | 10 | short |',
  '| Apple | 2 | a somewhat longer note that wraps |',
  '| Cherry | 30 | another |',
  '| Date | 7 | yet another row |',
].join('\n')

function textBlock(id: string, text: string, seq = 0): MessageContent {
  return {
    id: `${id}-c${seq}`,
    message_id: id,
    content_type: 'text',
    content: { type: 'text', text },
    sequence_order: seq,
    created_at: NOW,
    updated_at: NOW,
  }
}

function imageResourceBlock(id: string): MessageContent {
  return {
    id: `${id}-file`,
    message_id: id,
    content_type: 'tool_result',
    content: {
      type: 'tool_result',
      tool_use_id: `${id}-tu`,
      content: '',
      resource_links: [
        {
          uri: `${PNG_IMG}#${id}`,
          name: `chart-${id}.png`,
          mime_type: 'image/png',
        },
      ],
    },
    sequence_order: 1,
    created_at: NOW,
    updated_at: NOW,
  }
}

function buildMessages(n: number): Map<string, MessageWithContent> {
  const map = new Map<string, MessageWithContent>()
  for (let i = 0; i < n; i++) {
    const id = `g-msg-${i}`
    const role = i % 2 === 0 ? 'user' : 'assistant'
    const contents: MessageContent[] = []
    if (i % 13 === 0 && role === 'assistant') {
      contents.push(textBlock(id, `Here is a generated file (message ${i}).`))
      contents.push(imageResourceBlock(id))
    } else if (i % 7 === 0) {
      contents.push(textBlock(id, `Message ${i}. ${LONG_TEXT}`))
    } else if (i % 5 === 0) {
      contents.push(textBlock(id, `Message ${i} with a table:\n\n${MD_TABLE}`))
    } else if (i % 11 === 0) {
      contents.push(textBlock(id, `Message ${i} with an image:\n\n![chart](${SVG_IMG})`))
    } else {
      contents.push(textBlock(id, `Short message ${i}: a normal chat line.`))
    }
    map.set(id, {
      id,
      role,
      contents,
      originated_from_id: '',
      edit_count: 0,
      created_at: NOW,
      model_id: 'claude-opus-4-8',
    } as MessageWithContent)
  }
  return map
}

export function MessageListLongDemo({ count = 500 }: { count?: number }) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const listRef = useRef<MessageListHandle>(null)
  const [ready, setReady] = useState(false)

  useEffect(() => {
    useMessageViewStateStore.getState().resetViewState()
    const patch: Partial<ReturnType<typeof useChatStore.getState>> = {
      messages: buildMessages(count),
      loading: false,
      isStreaming: false,
      loadingOlder: false,
    }
    useChatStore.setState(patch)
    setReady(true)
  }, [count])

  return (
    <div className="flex w-full flex-col gap-2">
      {/* Jump control — drives MessageList.scrollToMessageId so the e2e can
          assert jump-to-message still lands + settles (TEST-11). */}
      <Button
        variant="outline"
        size="default"
        className="self-start"
        data-testid="g-msglist-jump"
        onClick={() => listRef.current?.scrollToMessageId('g-msg-250', 'center')}
      >
        Jump to message 250
      </Button>
      <div
        ref={scrollRef}
        data-testid="g-msglist-scroll"
        className="h-[600px] w-full overflow-y-auto border border-border bg-background"
      >
        <div className="mx-auto w-full max-w-4xl px-4">
          <MessageList
            ref={listRef}
            getScrollElement={() => scrollRef.current}
            scrollerReady={ready}
            virtualize
          />
        </div>
      </div>
    </div>
  )
}
