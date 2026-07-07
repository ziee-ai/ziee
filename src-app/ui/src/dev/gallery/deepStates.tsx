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
import { type InteractionRecipe, useRunInteraction } from './interactions'
import { holdForever } from './seeded/helpers'
import { ModelPicker } from '@/modules/user-llm-providers/ModelPicker.store'
import {
  BRANCHED_ANCHOR_MESSAGE_ID,
  BRANCHED_BRANCH_IDS,
  CHAT_DEEP_CONVERSATION_IDS,
  RENDERING_SHOWCASE_ID,
  SHOWCASE_CONVERSATION_ID,
  STREAMING_MESSAGE_ID,
  literaturePanelData,
  liveElicitation,
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
  /** Interaction recipes: drive real user actions after mount to render the
   *  interaction-gated states (click-to-edit, expand, hover) this surface hides
   *  behind a click. Driven via `?surface=<slug>&interact=<name>`. */
  interactions?: InteractionRecipe[]
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
    note: 'mid-generation: a streamingMessage carrying the accumulated deltas, left visibly streaming (isStreaming)',
    setup: async () => {
      await whenLoaded(SHOWCASE_CONVERSATION_ID)
      // Build the mid-generation state DIRECTLY (idempotent `setState` replace)
      // rather than replaying the recorded frames token-by-token through
      // `applyStreamFrame`. React StrictMode invokes effects twice in dev, and
      // the reducer's APPEND semantics double/interleave under a concurrent
      // re-invocation → garbled, duplicated text. A single deterministic replace
      // produces the same visible mid-stream state and is re-invocation-safe.
      const fullText = streamingCassette
        .filter(f => f.type === 'content')
        .flatMap(f => f.content ?? [])
        .map(c => c.delta)
        .join('')
      const now = new Date().toISOString()
      const streamingMessage = {
        id: STREAMING_MESSAGE_ID,
        role: 'assistant' as const,
        contents: [
          {
            id: `${STREAMING_MESSAGE_ID}-c0`,
            message_id: STREAMING_MESSAGE_ID,
            content_type: 'text',
            content: { type: 'text', text: fullText },
            sequence_order: 0,
            created_at: now,
            updated_at: now,
          },
        ],
        originated_from_id: '',
        edit_count: 0,
        created_at: now,
        model_id: 'claude-opus-4-8',
      }
      useChatStore.setState(s => {
        const messages = new Map(s.messages)
        messages.set(STREAMING_MESSAGE_ID, streamingMessage as never)
        return {
          messages,
          streamingMessage: streamingMessage as never,
          streamingMessageId: STREAMING_MESSAGE_ID,
          isStreaming: true,
        }
      })
    },
    interactions: [
      {
        // The composer "+" dropdown menu renders its items ONLY while open
        // (a Popover). Mount-only capture never shows them, so the A9 peer-icon
        // check couldn't scan the menu — where the "Skills in this chat" item's
        // BookOpen icon renders at lucide's 24px default (className="text-base"
        // doesn't resize an svg) vs the 16px (size-4) icons of its peers. This
        // recipe opens the menu so geometry + crop review see the item rows.
        name: 'open-plus-menu',
        note: 'click the composer "+" button → the open tools/files dropdown (mcp / skills / assistant menu-item rows)',
        steps: async d => {
          await d.click('chat-input-add-btn')
          await d.wait(400)
        },
      },
    ],
  },
  {
    // H7 empty-control coverage: the composer model picker (`ullm-model-select`)
    // with ZERO models. ModelPicker is held empty (providers=[]) while the
    // conversation still carries a stale model id → the Select has a value it
    // can't resolve against 0 options + no placeholder (suppressed once a value is
    // set) → the trigger renders LITERALLY BLANK (no value, no "No models" hint,
    // no configure affordance). This state only exists after a store seed — the
    // enumerated composer always has models.
    slug: 'deep-chat-no-models',
    title: 'Conversation — composer with no models configured',
    conversationId: SHOWCASE_CONVERSATION_ID,
    note: 'ModelPicker held empty (providers=[]) + stale selectedModelId → the composer model-select renders blank (H7 empty-control-renders-nothing)',
    setup: async () => {
      await whenLoaded(SHOWCASE_CONVERSATION_ID)
      // holdForever re-asserts every 150ms so the composer's own loadProviders()
      // (which refills from the cassette on mount) can't repopulate it.
      holdForever(() =>
        ModelPicker.store.setState({
          providers: [],
          selectedModelId: null,
          loading: false,
          error: null,
        } as never),
      )
    },
    interactions: [
      {
        // Open the model picker so H7 can see the OPEN dropdown: with 0 models it
        // renders a listbox with ZERO options and NO "No models" empty-hint — it
        // shows literally nothing to select.
        name: 'open-model-select',
        note: 'click the composer model select → the (empty) listbox: 0 options, no "No models" hint (H7)',
        steps: async d => {
          await d.click('ullm-model-select')
          await d.wait(400)
        },
      },
    ],
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
    // Live MCP-composer tool-call in COMPLETED status: seeding McpComposer.toolCalls
    // for the running conversation's tool_use block id makes McpToolCallUI render
    // the completed status marker (mcp/chat-extension/extension.tsx:69/70).
    slug: 'deep-chat-mcp-toolcall-completed',
    title: 'Conversation — MCP tool call completed',
    conversationId: CHAT_DEEP_CONVERSATION_IDS.toolRunning,
    note: 'McpComposer toolCall in completed status → the sr-only completed marker',
    setup: async () => {
      await whenLoaded(CHAT_DEEP_CONVERSATION_IDS.toolRunning)
      useMcpComposerStore.getState().addToolCall({
        tool_use_id: 'toolu_running_1',
        server: 'code_sandbox',
        tool_name: 'execute_command',
        status: 'completed',
        result: { ok: true },
      })
    },
  },
  {
    // Live MCP-composer tool-call in ERROR status: seeds McpComposer.toolCalls for
    // the failed conversation's tool_use id → McpToolCallUI renders the error Alert
    // (extension.tsx:132/133) + the aggregate hasError icon (:294).
    slug: 'deep-chat-mcp-toolcall-error',
    title: 'Conversation — MCP tool call errored',
    conversationId: CHAT_DEEP_CONVERSATION_IDS.toolFailed,
    note: 'McpComposer toolCall in error status → the error Alert + CircleX icon',
    setup: async () => {
      await whenLoaded(CHAT_DEEP_CONVERSATION_IDS.toolFailed)
      useMcpComposerStore.getState().addToolCall({
        tool_use_id: 'toolu_failed_1',
        server: 'code_sandbox',
        tool_name: 'execute_command',
        status: 'error',
        error: 'Tool call failed: exit code 1.',
      })
    },
    interactions: [
      {
        // The tool-call card's error Alert + expanded detail panel render ONLY
        // inside `{isExpanded && …}` (mcp/chat-extension/extension.tsx:112) — a
        // click on the details chevron. This is exactly the interaction the
        // coverage-allowlist excused for extension.tsx:132/133; the recipe drives
        // it so the branch is exercised (de-allowlisted) + the expanded state shot.
        name: 'expand-details',
        note: 'click the tool-call details chevron → the expanded error Alert + arguments panel (extension.tsx:112/132/133)',
        steps: async d => {
          await d.click('mcp-toolcall-details-btn-toolu_failed_1')
          await d.wait(300)
        },
      },
    ],
  },
  {
    // Priority "must render" state: the inline tool-approval prompt. Seeding a
    // McpComposer toolCall in `pending_approval` for the running conversation's
    // tool_use block makes McpToolCallUI render ToolCallPendingApprovalContent —
    // the "Tool Approval Required" Alert + approve/deny buttons (a state the
    // mount-only pass never showed; the C9/C10 icon-alignment bug family lives here).
    slug: 'deep-chat-tool-approval',
    title: 'Conversation — tool approval pending',
    conversationId: CHAT_DEEP_CONVERSATION_IDS.toolRunning,
    note: 'McpComposer toolCall in pending_approval → the inline "Tool Approval Required" prompt (approve-once / approve-conv / deny)',
    setup: async () => {
      await whenLoaded(CHAT_DEEP_CONVERSATION_IDS.toolRunning)
      useMcpComposerStore.getState().addToolCall({
        tool_use_id: 'toolu_running_1',
        server: 'code_sandbox',
        server_id: 'a1b2c3d4-0000-5000-8000-000000000001',
        tool_name: 'execute_command',
        status: 'pending_approval',
        input: { command: 'ls -la /workspace' },
      })
    },
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
    conversationId: CHAT_DEEP_CONVERSATION_IDS.elicitation,
    note: 'a dedicated conversation ending in a pending elicitation_request → the live, answerable form',
    setup: async () => {
      await whenLoaded(CHAT_DEEP_CONVERSATION_IDS.elicitation)
      // The block's own `status: 'pending'` already renders the form; seeding the
      // McpComposer live entry (matching id) makes it the freshest-status source too.
      useMcpComposerStore.getState().addElicitationRequest(liveElicitation)
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
    // MULTI-tab right panel: opening two tabs surfaces the real tab STRIP (a
    // horizontal tablist with >1 tab) so the strip detectors have a target —
    // A8 (row-child vertical centering) + I5 (wrong-scroll-axis). A single-tab
    // panel renders no strip.
    slug: 'deep-chat-right-panel-multi',
    title: 'Conversation — right panel, multiple tabs',
    conversationId: SHOWCASE_CONVERSATION_ID,
    note: 'two right-panel tabs (file + literature) → the tab strip; drives A8/I5',
    setup: async () => {
      await whenLoaded(SHOWCASE_CONVERSATION_ID)
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
      chat().displayInRightPanel({
        id: literaturePanelData.sessionId,
        title: 'Literature screening',
        type: 'literature',
        data: literaturePanelData,
      })
    },
  },
  {
    // RENDERING SHOWCASE: a conversation whose one assistant message carries
    // math / mermaid / a highlighted code fence / a table. Feeds the Layer-1
    // content-rendering detectors (L1/L2/L3/L4) so the audit reports whether each
    // rich renderer works in the gallery or degrades to raw text.
    slug: 'deep-chat-rendering-showcase',
    title: 'Conversation — rendering showcase (math/mermaid/code/table)',
    conversationId: RENDERING_SHOWCASE_ID,
    note: 'math (KaTeX) + mermaid + highlighted code + table → drives L1/L2/L3/L4',
    setup: async () => {
      await whenLoaded(RENDERING_SHOWCASE_ID)
    },
  },
  {
    slug: 'deep-chat-branched',
    title: 'Conversation — branched (edit/regenerate branches)',
    conversationId: CHAT_DEEP_CONVERSATION_IDS.branched,
    note: 'a fork point on the (visible) last assistant message → the BranchNavigator < 1 / 3 >',
    setup: async () => {
      await whenLoaded(CHAT_DEEP_CONVERSATION_IDS.branched)
      // `forkPoints` is normally derived by loadBranches from a parent/child branch
      // graph; seed it directly (a store field) so the navigator renders on the
      // visible anchor message without hand-crafting that graph.
      useChatStore.setState(s => {
        const forkPoints = new Map(s.forkPoints)
        forkPoints.set(BRANCHED_ANCHOR_MESSAGE_ID, [...BRANCHED_BRANCH_IDS])
        return { forkPoints }
      })
    },
  },
  {
    slug: 'deep-chat-long',
    title: 'Conversation — long history (scroll)',
    conversationId: SHOWCASE_CONVERSATION_ID,
    note: '47-message showcase history — scroll + lazy-preview behavior',
    setup: () => whenLoaded(SHOWCASE_CONVERSATION_ID),
    interactions: [
      {
        // THE flagship interaction bug: the conversation-header inline rename form.
        // Clicking the pencil sets TitleEditor.isEditing → the `<Form>` swaps in.
        // KNOWN BUG (A10): the inline rename form renders VERTICAL and the input
        // collapses to invisible width — a state the mount-only pass never showed.
        name: 'rename',
        note: 'click the header edit pencil → the inline rename Form (A10: input collapses to invisible width / vertical layout)',
        steps: async d => {
          await d.click('chat-title-edit-btn')
          await d.waitFor('chat-title-input', 3000)
          await d.wait(300)
        },
      },
      {
        // MessageActions is `opacity-0 group-hover/group-focus-within:opacity-100`.
        // A synthetic pointer event can't fire CSS :hover, but focusing a button in
        // the row triggers `focus-within` → the same reveal (G2 hover/G7 focus ring).
        name: 'message-actions',
        note: 'focus a message action → the hover/focus-revealed action row (copy / edit / regenerate) becomes visible',
        steps: async d => {
          await d.focus('chat-message-copy-btn')
          await d.wait(300)
        },
      },
    ],
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
  // Deep surfaces need their seed + the lazy ConversationPage to settle before an
  // interaction can find its target, so give the recipe a longer settle window.
  useRunInteraction(entry.interactions, 1200)
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
