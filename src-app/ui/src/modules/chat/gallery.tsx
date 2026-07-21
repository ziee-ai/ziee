/**
 * Dev-gallery seed for the `chat` module — the active-conversation deep-states
 * (streaming / tool-running / tool-approval / attachments / elicitation /
 * right-panel / branched / rendering-showcase / long history) plus the seeded
 * real-component surfaces (recent-chats widget, ChatMessage, MessageList,
 * ChatHistoryPage, ConversationPage loading/not-found/error).
 *
 * Auto-discovered by the gallery's runtime registry (`@/dev/gallery/support`);
 * never imported by `module.tsx`, so it is dev-only and tree-shaken from prod.
 *
 * `ConversationPage` / `DeepStateFrame` / `SeededSurfaceFrame` (the renderers)
 * stay in the central gallery aggregator; this file only carries the entries.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import {
  holdForever,
  holdPatch,
  lazyNamed,
  lazyProps,
  whenTrue,
} from '@/dev/gallery/support'
import { chatCassette } from '@/dev/gallery/fixtures/chat'
import { useChatStore } from '@/modules/chat/core/stores/chat'
import { useFileStore } from '@/modules/file/stores/File.store'
import { useMcpComposerStore } from '@/modules/mcp/stores/McpComposer.store'
import { useModelPickerStore } from '@/modules/user-llm-providers/modelPicker'
import {
  BRANCHED_ANCHOR_MESSAGE_ID,
  BRANCHED_BRANCH_IDS,
  CHAT_DEEP_CONVERSATION_IDS,
  RENDERING_SHOWCASE_ID,
  SHOWCASE_CONVERSATION_ID,
  STREAMING_MESSAGE_ID,
  literaturePanelData,
  liveAskUser,
  liveElicitation,
  rightPanelFile,
  streamingCassette,
} from '@/dev/gallery/fixtures/chat-deep'

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

/** Build N mock recent conversations for the RecentConversationsWidget seeds
 * (loaded / loading-more) — enough rows that the virtualized list windows. */
function mkRecentConvos(n: number) {
  const now = Date.now()
  return Array.from({ length: n }, (_, i) => ({
    id: `gallery-recent-${i}`,
    title: `Recent conversation ${i + 1}`,
    user_id: 'u1',
    created_at: new Date(now - i * 60_000).toISOString(),
    updated_at: new Date(now - i * 60_000).toISOString(),
    message_count: (i % 7) + 1,
  }))
}

export const gallery: ModuleGallery = {
  cassette: chatCassette,
  deepStates: [
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
          useModelPickerStore.setState({
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
      // A ≥2-tool run folded into the "N tools called" group card, with an
      // artifact tool_result that was persisted non-adjacent to its tool_use.
      // normalizeToolResultOrder wraps it inside the group and the group
      // auto-opens for the artifact, so the inline preview shows without a click.
      slug: 'deep-chat-tool-group',
      title: 'Conversation — tool group with artifact (auto-open)',
      conversationId: CHAT_DEEP_CONVERSATION_IDS.toolGroup,
      note: 'a 2-tool group card, auto-expanded because a run tool_result carries an artifact',
      setup: () => whenLoaded(CHAT_DEEP_CONVERSATION_IDS.toolGroup),
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
      slug: 'deep-chat-ask-user-wizard',
      title: 'Conversation — ask_user decision wizard (rich)',
      conversationId: CHAT_DEEP_CONVERSATION_IDS.askUser,
      note: 'the ziee-internal ask_user rich UX: a 2-question wizard of option cards with descriptions, a recommended badge, an inline preview, and the Other escape',
      setup: async () => {
        await whenLoaded(CHAT_DEEP_CONVERSATION_IDS.askUser)
        useMcpComposerStore.getState().addElicitationRequest(liveAskUser)
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
      title: 'Conversation — rendering showcase (math/mermaid/code/html/table)',
      conversationId: RENDERING_SHOWCASE_ID,
      note: 'math (KaTeX) + mermaid + highlighted code + html block + table → drives L1/L2/L3/L4',
      setup: async () => {
        await whenLoaded(RENDERING_SHOWCASE_ID)
      },
      interactions: [
        {
          // The ```html block defaults to the CODE view; clicking the toggle's
          // Preview option flips it to the sandboxed-iframe render — captures the
          // render-mode combo (the mount-only pass only shows the default CODE mode).
          name: 'html-preview',
          note: 'click the HTML block Code⇄Preview toggle → the sandboxed-iframe live render (default is CODE)',
          steps: async d => {
            await d.click('html-block-toggle-opt-preview')
            await d.waitFor('html-block-preview', 3000)
            await d.wait(300)
          },
        },
      ],
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
  ],
  seeded: [
    {
      slug: 'seeded-recent-convos-loading',
      title: 'Recent chats widget — loading',
      note: '!recentInitialized → the loading spinner',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/chat/widgets/RecentConversationsWidget'),
        'RecentConversationsWidget',
      ),
      setup: async () => {
        const { useChatHistoryStore } = await import(
          '@/modules/chat/stores/chatHistory'
        )
        await holdPatch(() =>
          useChatHistoryStore.setState({
            recentLoading: true,
            recentInitialized: false,
          } as any),
        )
      },
    },
    // ── RecentConversationsWidget: empty (recentInitialized && no rows). ─────────
    {
      slug: 'seeded-recent-convos-empty',
      title: 'Recent chats widget — empty',
      note: 'recentInitialized && recentConversations.length===0 → empty state',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/chat/widgets/RecentConversationsWidget'),
        'RecentConversationsWidget',
      ),
      setup: async () => {
        const { useChatHistoryStore } = await import(
          '@/modules/chat/stores/chatHistory'
        )
        await holdPatch(() =>
          useChatHistoryStore.setState({
            recentInitialized: true,
            recentLoading: false,
            recentConversations: [],
          } as any),
        )
      },
    },
    // ── RecentConversationsWidget: loaded (many, has more → virtualized list). ───
    {
      slug: 'seeded-recent-convos-loaded',
      title: 'Recent chats widget — loaded (many)',
      note: 'recentInitialized + 40 rows of 45 → virtualized infinite-scroll list',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/chat/widgets/RecentConversationsWidget'),
        'RecentConversationsWidget',
      ),
      setup: async () => {
        const { useChatHistoryStore } = await import(
          '@/modules/chat/stores/chatHistory'
        )
        await holdPatch(() =>
          useChatHistoryStore.setState({
            recentInitialized: true,
            recentLoading: false,
            recentLoadingMore: false,
            recentConversations: mkRecentConvos(40),
            recentTotal: 45,
            recentHasMore: true,
            recentPage: 2,
          } as any),
        )
      },
    },
    // ── RecentConversationsWidget: first-load error (retryable). ────────────────
    {
      slug: 'seeded-recent-convos-error',
      title: 'Recent chats widget — error',
      note: 'recentError && no rows → the retryable error state',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/chat/widgets/RecentConversationsWidget'),
        'RecentConversationsWidget',
      ),
      setup: async () => {
        const { useChatHistoryStore } = await import(
          '@/modules/chat/stores/chatHistory'
        )
        await holdPatch(() =>
          useChatHistoryStore.setState({
            recentInitialized: false,
            recentLoading: false,
            recentConversations: [],
            recentError: 'Failed to load conversations',
          } as any),
        )
      },
    },
    // ── RecentConversationsWidget: loading a further page (bottom spinner). ──────
    {
      slug: 'seeded-recent-convos-loading-more',
      title: 'Recent chats widget — loading more',
      note: 'recentLoadingMore → the bottom "Loading more" indicator',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/chat/widgets/RecentConversationsWidget'),
        'RecentConversationsWidget',
      ),
      setup: async () => {
        const { useChatHistoryStore } = await import(
          '@/modules/chat/stores/chatHistory'
        )
        await holdPatch(() =>
          useChatHistoryStore.setState({
            recentInitialized: true,
            recentLoading: false,
            recentLoadingMore: true,
            recentConversations: mkRecentConvos(20),
            recentTotal: 45,
            recentHasMore: true,
            recentPage: 1,
          } as any),
        )
      },
    },
    // ── ConversationListLongDemo: ≈200 conversations driving the REAL virtualized
    //    ConversationList in a fixed-height scroll box → the chats-page
    //    virtualization window / scroll / no-jank surface. Backend-free rows.
    {
      slug: 'seeded-conversation-list-long',
      title: 'Chats list — long virtualized conversation list (interactive)',
      note: '≈200 mixed-height conversation cards → row-virtualization window/scroll/no-jank surface',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/dev/gallery/ConversationListLongDemo'),
        'ConversationListLongDemo',
        { count: 200 },
      ),
    },
    {
      slug: 'seeded-conversation-list-long-narrow',
      title: 'Chats list — virtualized list at narrow (390px) width',
      note: '≈200 conversation cards constrained to a 390px mobile-width column (responsive-fidelity)',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/dev/gallery/ConversationListLongDemo'),
        'ConversationListLongDemo',
        { count: 200, narrow: true },
      ),
    },
    // ── ChatMessage: a message with no content blocks → the `return null` arm. ───
    {
      slug: 'seeded-chat-message-empty',
      title: 'Chat message — no contents',
      note: '!message.contents || length===0 → renders nothing',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/chat/components/ChatMessage'),
        'ChatMessage',
        {
          message: {
            id: 'gallery-empty-msg',
            role: 'assistant',
            contents: [],
            originated_from_id: '',
            edit_count: 0,
            created_at: new Date().toISOString(),
            model_id: 'claude-opus-4-8',
          },
        },
      ),
    },
    // ── MessageList: a loaded conversation with zero messages → the empty state. ─
    {
      slug: 'seeded-message-list-empty',
      title: 'Message list — empty conversation',
      note: '!loading && messagesArray.length===0 → the empty conversation state',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/chat/components/MessageList'),
        'MessageList',
      ),
      setup: async () => {
        const { useChatStore } = await import(
          '@/modules/chat/core/stores/chat'
        )
        await holdPatch(() =>
          useChatStore.setState({
            messages: new Map(),
            loading: false,
            isStreaming: false,
          } as any),
        )
      },
    },
    // ── ChatHistoryPage: the list-shown arm (conversations>0 || loading || error). ─
    {
      slug: 'seeded-chat-history-list',
      title: 'Chat history — list shown (loading)',
      note: 'loading && !isInitialized → the ConversationList load spinner (container mounted via the loading arm)',
      path: '/chat-history',
      initialPath: '/chat-history',
      // ChatHistoryPage is a DEFAULT export — `lazyNamed(…, 'ChatHistoryPage')`
      // resolved to `undefined` (blank via the boundary). Load the default.
      component: lazyNamed(
        () => import('@/modules/chat/pages/ChatHistoryPage'),
        'default',
      ),
      setup: async () => {
        const { useChatHistoryStore } = await import(
          '@/modules/chat/stores/chatHistory'
        )
        const { AppLayoutDef } = await import(
          '@/modules/layouts/app-layout/appLayout'
        )
        // ChatHistoryPage refetches on mount (which flips loading/isInitialized as
        // it resolves), so a one-shot seed races into a blank window: `loading`
        // (mid-fetch) with a seeded `isInitialized:true` matches NEITHER the error
        // arm (needs !loading) NOR the spinner arm (needs !isInitialized) → blank.
        // Assert a persistent loading state (`holdForever`) so the container mounts
        // via the loading arm (also covering the `nativeScroll===true` :143 ternary)
        // and ConversationList deterministically shows its load spinner.
        holdForever(() => {
          AppLayoutDef.store.setState({ nativeScroll: true } as any)
          useChatHistoryStore.setState({
            loading: true,
            isInitialized: false,
            conversations: [],
            error: null,
          } as any)
        })
      },
    },
    // ── ConversationPage: still-loading (loading && !conversation). ──────────────
    // The GET-driven pass can't hold a page mid-load; seed loading:true + no
    // conversation so the `<Loading/>` early return (line 101) renders.
    {
      slug: 'seeded-s5-conversation-loading',
      title: 'Conversation page — loading',
      note: 'loading && !conversation → the page load spinner (ConversationPage:101)',
      path: '/chat/:conversationId',
      initialPath: '/chat/s5-loading',
      component: lazyNamed(
        () => import('@/modules/chat/pages/ConversationPage'),
        'default',
      ),
      setup: async () => {
        const { useChatStore } = await import(
          '@/modules/chat/core/stores/chat'
        )
        await holdPatch(() =>
          useChatStore.setState({ loading: true, conversation: null } as any),
        )
      },
    },
    // ── ConversationPage: not-found (!loading && !conversation). ─────────────────
    {
      slug: 'seeded-s5-conversation-not-found',
      title: 'Conversation page — not found',
      note: '!loading && !conversation → "Conversation not found" alert (ConversationPage:108)',
      path: '/chat/:conversationId',
      initialPath: '/chat/s5-missing',
      component: lazyNamed(
        () => import('@/modules/chat/pages/ConversationPage'),
        'default',
      ),
      setup: async () => {
        const { useChatStore } = await import(
          '@/modules/chat/core/stores/chat'
        )
        await holdPatch(() =>
          useChatStore.setState({
            loading: false,
            conversation: null,
            error: null,
          } as any),
        )
      },
    },
    // ── ConversationPage: loaded conversation + a send/stream error banner. ──────
    // Load the real showcase conversation (passes the two !conversation early
    // returns), then seed `error` so the inline error banner (line 142) renders.
    {
      slug: 'seeded-s5-conversation-error',
      title: 'Conversation page — error banner',
      note: 'conversation loaded + Stores.Chat.error → the inline error banner (ConversationPage:142)',
      path: '/chat/:conversationId',
      initialPath: '/chat/11111111-1111-1111-1111-111111111111',
      component: lazyNamed(
        () => import('@/modules/chat/pages/ConversationPage'),
        'default',
      ),
      setup: async () => {
        const { useChatStore } = await import(
          '@/modules/chat/core/stores/chat'
        )
        const { SHOWCASE_CONVERSATION_ID } = await import(
          '@/dev/gallery/fixtures/chat-deep'
        )
        await useChatStore.getState().loadConversation(SHOWCASE_CONVERSATION_ID)
        await whenTrue(
          () =>
            useChatStore.getState().conversation?.id === SHOWCASE_CONVERSATION_ID,
        )
        // setState merges shallow → conversation is preserved, only error/loading flip.
        await holdPatch(() =>
          useChatStore.setState({
            error: 'Failed to send message. Please try again.',
            loading: false,
          } as any),
        )
      },
    },
  ],
}
