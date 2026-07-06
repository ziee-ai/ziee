/**
 * Active-conversation DEEP-STATE entries — the states the seeded gallery's
 * agent-authored list missed on the chat page. Each renders the REAL
 * `ConversationPage` inside an isolated `MemoryRouter` pinned to a fixture
 * conversation, then a `setup()` seeds the transient piece through the REAL Chat
 * store path (no bespoke render): a right-panel tab, a mid-generation stream, a
 * pending elicitation. Rendered one-per-page-load via `?surface=<slug>` — like
 * overlays — so the single-active Chat singleton never bleeds across entries.
 *
 * These go BEYOND the tsc-gated required-state matrix (streaming / tool-running
 * have no single named RequiredState key); Part 2's branch coverage is what
 * proves they actually exercise the MessageList / ContentRenderer branches.
 */
import { type ReactNode, Suspense, lazy, useEffect } from 'react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { Text, Title } from '@/components/ui'
import { AppErrorBoundary } from '@/components/AppErrorBoundary'
import { Loading } from '@/core/components/Loading'
import { useChatStore } from '@/modules/chat/core/stores/Chat.store'
import { useFileStore } from '@/modules/file/stores/File.store'
import { useMcpComposerStore } from '@/modules/mcp/stores/McpComposer.store'
import { setSseCassette } from './mockApi'
import {
  CHAT_DEEP_CONVERSATION_IDS,
  SHOWCASE_CONVERSATION_ID,
  STREAMING_MESSAGE_ID,
  literaturePanelData,
  pendingElicitation,
  rightPanelFile,
  streamingCassette,
} from './fixtures/chat-deep'

const ConversationPage = lazy(
  () => import('@/modules/chat/pages/ConversationPage'),
)

export interface DeepStateEntry {
  /** Gallery slug → `?surface=<slug>`; also the section testid. */
  slug: string
  /** Human title for the frame. */
  title: string
  /** Which conversation the ConversationPage is pinned to. */
  conversationId: string
  /** One-line note about what deep state this exercises. */
  note: string
  /** Seed the transient state through the real store (runs after mount). */
  setup?: () => void | Promise<void>
}

const chat = () => useChatStore.getState()
const tick = (ms = 120) => new Promise(r => setTimeout(r, ms))

/** Ensure the pinned conversation is loaded before layering transient state. */
async function whenLoaded(conversationId: string): Promise<void> {
  await chat().loadConversation(conversationId)
  for (let i = 0; i < 40; i++) {
    if (chat().conversation?.id === conversationId) return
    await tick(50)
  }
}

export const DEEP_STATE_ENTRIES: DeepStateEntry[] = [
  {
    slug: 'deep-chat-streaming',
    title: 'Conversation — streaming (live generation)',
    conversationId: SHOWCASE_CONVERSATION_ID,
    note: 'mid-generation: streamingMessage assembled token-by-token via the real applyStreamFrame reducer',
    setup: async () => {
      // Register the recorded SSE frames so the mechanism is exercised/available;
      // then drive the SAME frames through the real reducer directly (robust —
      // the gallery does not boot the auth-gated ChatStreamClient).
      setSseCassette(
        streamingCassette.map(f => ({ event: f.type, data: { conversationId: SHOWCASE_CONVERSATION_ID, event: f } })),
      )
      await whenLoaded(SHOWCASE_CONVERSATION_ID)
      for (const frame of streamingCassette) {
        await chat().applyStreamFrame(SHOWCASE_CONVERSATION_ID, frame)
        await tick(250)
      }
      // Leave it mid-stream (no `complete`) so isStreaming stays true.
      useChatStore.setState({ isStreaming: true, streamingMessageId: STREAMING_MESSAGE_ID })
    },
  },
  {
    slug: 'deep-chat-tool-running',
    title: 'Conversation — tool call running',
    conversationId: CHAT_DEEP_CONVERSATION_IDS.toolRunning,
    note: 'a tool_use block with no paired tool_result yet',
    setup: () => whenLoaded(CHAT_DEEP_CONVERSATION_IDS.toolRunning),
  },
  {
    slug: 'deep-chat-tool-failed',
    title: 'Conversation — tool call failed',
    conversationId: CHAT_DEEP_CONVERSATION_IDS.toolFailed,
    note: 'a tool_result with is_error: true',
    setup: () => whenLoaded(CHAT_DEEP_CONVERSATION_IDS.toolFailed),
  },
  {
    slug: 'deep-chat-attachments',
    title: 'Conversation — message with attachments',
    conversationId: CHAT_DEEP_CONVERSATION_IDS.attachments,
    note: 'file_attachment + image content blocks on a user message',
    setup: () => whenLoaded(CHAT_DEEP_CONVERSATION_IDS.attachments),
  },
  {
    slug: 'deep-chat-elicitation',
    title: 'Conversation — elicitation prompt pending',
    conversationId: SHOWCASE_CONVERSATION_ID,
    note: 'the elicitation_request block flips to a live, answerable form',
    setup: async () => {
      await whenLoaded(SHOWCASE_CONVERSATION_ID)
      useMcpComposerStore.getState().addElicitationRequest(pendingElicitation)
    },
  },
  {
    slug: 'deep-chat-right-panel-file',
    title: 'Conversation — right panel open (file viewer)',
    conversationId: SHOWCASE_CONVERSATION_ID,
    note: 'a file tab opened in the right panel (registerPanelRenderer("file"))',
    setup: async () => {
      await whenLoaded(SHOWCASE_CONVERSATION_ID)
      // Seed the file into the File store so the file panel resolves it.
      useFileStore.setState(s => ({
        selectedFiles: new Map(s.selectedFiles).set(rightPanelFile.id, rightPanelFile),
        messageFilesCache: new Map(s.messageFilesCache).set(rightPanelFile.id, rightPanelFile),
      }))
      chat().displayInRightPanel({
        id: 'panel-file-1',
        title: rightPanelFile.filename,
        type: 'file',
        data: { fileId: rightPanelFile.id },
      })
    },
  },
  {
    slug: 'deep-chat-right-panel-literature',
    title: 'Conversation — right panel open (literature screening)',
    conversationId: SHOWCASE_CONVERSATION_ID,
    note: 'a literature screening tab (registerPanelRenderer("literature"))',
    setup: async () => {
      await whenLoaded(SHOWCASE_CONVERSATION_ID)
      chat().displayInRightPanel({
        id: literaturePanelData.sessionId,
        title: 'Literature screening',
        type: 'literature',
        data: literaturePanelData,
      })
    },
  },
  {
    slug: 'deep-chat-branched',
    title: 'Conversation — branched (edit/regenerate branches)',
    conversationId: SHOWCASE_CONVERSATION_ID,
    note: 'the showcase conversation carries edit + regenerate branches → BranchNavigator',
    setup: () => whenLoaded(SHOWCASE_CONVERSATION_ID),
  },
  {
    slug: 'deep-chat-long',
    title: 'Conversation — long history (scroll)',
    conversationId: SHOWCASE_CONVERSATION_ID,
    note: '47-message showcase history — scroll + lazy-preview behavior',
    setup: () => whenLoaded(SHOWCASE_CONVERSATION_ID),
  },
]

export const deepStateBySlug = (slug: string) =>
  DEEP_STATE_ENTRIES.find(e => e.slug === slug)

/** Surface ids each deep entry helps cover (for reference/reporting). */
export const DEEP_STATE_SLUGS = DEEP_STATE_ENTRIES.map(e => e.slug)

const deepTestId = (slug: string) => `gallery-page-${slug}`

/** Renders one deep-state entry: the real ConversationPage + a mount-time seed. */
export function DeepStateFrame({ entry }: { entry: DeepStateEntry }): ReactNode {
  useEffect(() => {
    void entry.setup?.()
  }, [entry])
  return (
    <section
      data-testid={deepTestId(entry.slug)}
      data-gallery-state="deep"
      className="flex flex-col gap-3 border border-border rounded-lg p-4 bg-background"
    >
      <div className="flex flex-col gap-1" data-gallery-chrome>
        <Title level={3}>
          {entry.title}
          <Text tone="muted" className="ml-2 text-sm">
            · deep-state
          </Text>
        </Title>
        <Text tone="muted" className="text-sm">
          gallery-page-{entry.slug} · {entry.note}
        </Text>
      </div>
      <div
        className="w-full overflow-hidden rounded-md border border-border bg-background"
        style={{ height: 720 }}
      >
        <AppErrorBoundary label={`deep-${entry.slug}`} fallback={() => null}>
          <MemoryRouter initialEntries={[`/chat/${entry.conversationId}`]}>
            <Routes>
              <Route
                path="/chat/:conversationId"
                element={
                  <Suspense fallback={<Loading />}>
                    <ConversationPage />
                  </Suspense>
                }
              />
            </Routes>
          </MemoryRouter>
        </AppErrorBoundary>
      </div>
    </section>
  )
}
