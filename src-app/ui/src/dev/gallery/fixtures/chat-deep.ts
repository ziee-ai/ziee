/**
 * Chat DEEP-STATE fixtures — the active-conversation states the seeded gallery's
 * agent-authored list missed: a tool-call still RUNNING (no result yet), a FAILED
 * tool call, and attachments carried on a user message. Plus the transient-state
 * seeds (streaming SSE frames, a pending elicitation, right-panel payloads) that
 * `deepStates.tsx` drives through the REAL Chat store path.
 *
 * Everything here is typed against the generated api-client response types, so a
 * shape drift fails `tsc` (same correctness contract as the recorded cassettes).
 */
import type {
  Branch,
  Conversation,
  File as FileEntity,
  MessageContent,
  MessageContentData,
  MessageWithContent,
  SSEChatStreamMcpElicitationRequiredData,
} from '@/api-client/types'
import type { LiteratureScreeningData } from '@/modules/literature/types'

const NOW = '2026-07-05T20:16:49.884839Z'
const SANDBOX_SERVER = 'b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd'

/** The rich recorded showcase conversation — reused for the transient-state
 *  seeds (streaming / elicitation / right-panel) which layer over its history.
 *  Inlined (NOT imported from ./chat) so chat.ts → chat-deep.ts stays acyclic;
 *  a `gallery:check-fixtures` assertion guards this id against the recording. */
export const SHOWCASE_CONVERSATION_ID = '11111111-1111-1111-1111-111111111111'

// ── typed builders ───────────────────────────────────────────────────────────
let seq = 0
function block(messageId: string, data: MessageContentData): MessageContent {
  return {
    id: `${messageId}-c${seq++}`,
    message_id: messageId,
    content_type: data.type,
    content: data,
    sequence_order: 0,
    created_at: NOW,
    updated_at: NOW,
  }
}
function message(
  id: string,
  role: 'user' | 'assistant',
  blocks: MessageContentData[],
): MessageWithContent {
  seq = 0
  return {
    id,
    role,
    contents: blocks.map((b, i) => ({ ...block(id, b), sequence_order: i })),
    originated_from_id: '',
    edit_count: 0,
    created_at: NOW,
    model_id: 'claude-opus-4-8',
  }
}
function conversation(id: string, title: string): Conversation {
  return {
    id,
    title,
    user_id: 'aaaa0000-0000-0000-0000-000000000001',
    active_branch_id: `${id.slice(0, 8)}-branch-0001`,
    created_at: NOW,
    updated_at: NOW,
    model_id: 'claude-opus-4-8',
  }
}

export interface DeepBundle {
  conversation: Conversation
  messages: MessageWithContent[]
  branches: Branch[]
}

// ── synthetic conversations for states not present in the showcase ───────────

// A tool call still RUNNING: a `tool_use` block with NO paired `tool_result`.
const TOOL_RUNNING_ID = 'dee90001-0000-4000-8000-000000000001'
const toolRunning: DeepBundle = {
  conversation: conversation(TOOL_RUNNING_ID, 'Tool call — running'),
  messages: [
    message(`${TOOL_RUNNING_ID}-m1`, 'user', [
      { type: 'text', text: 'Run the analysis script and show me the output.' },
    ]),
    message(`${TOOL_RUNNING_ID}-m2`, 'assistant', [
      { type: 'text', text: 'Running the analysis now…' },
      {
        type: 'tool_use',
        id: 'toolu_running_1',
        name: 'execute_command',
        server_id: SANDBOX_SERVER,
        input: { command: 'python analyze.py --full', timeout_ms: 120000 },
      },
    ]),
  ],
  branches: [],
}

// A FAILED tool call: a `tool_result` with `is_error: true`.
const TOOL_FAILED_ID = 'dee90002-0000-4000-8000-000000000002'
const toolFailed: DeepBundle = {
  conversation: conversation(TOOL_FAILED_ID, 'Tool call — failed'),
  messages: [
    message(`${TOOL_FAILED_ID}-m1`, 'user', [
      { type: 'text', text: 'Install the missing dependency and retry.' },
    ]),
    message(`${TOOL_FAILED_ID}-m2`, 'assistant', [
      {
        type: 'tool_use',
        id: 'toolu_failed_1',
        name: 'execute_command',
        server_id: SANDBOX_SERVER,
        input: { command: 'pip install nonexistent-pkg-xyz', timeout_ms: 30000 },
      },
      {
        type: 'tool_result',
        content:
          'exit_code: 1\n--- stderr ---\nERROR: Could not find a version that satisfies the requirement nonexistent-pkg-xyz\nERROR: No matching distribution found for nonexistent-pkg-xyz',
        is_error: true,
        name: 'execute_command',
        server_id: SANDBOX_SERVER,
      } as MessageContentData,
    ]),
  ],
  branches: [],
}

// Attachments carried ON a user message (image + PDF) — the file/image content
// blocks that ride an outbound user turn.
const ATTACHMENTS_ID = 'dee90003-0000-4000-8000-000000000003'
const attachments: DeepBundle = {
  conversation: conversation(ATTACHMENTS_ID, 'Message with attachments'),
  messages: [
    message(`${ATTACHMENTS_ID}-m1`, 'user', [
      { type: 'text', text: 'Here are the chart and the report — summarize both.' },
      {
        type: 'file_attachment',
        file_id: 'f1000000-0000-0000-0000-000000000005',
        file_size: 631,
        filename: 'report.pdf',
        mime_type: 'application/pdf',
      },
      {
        type: 'image',
        alt_text: 'Bar chart PNG',
        source: { type: 'file', file_id: 'f1000000-0000-0000-0000-000000000001' },
      },
    ]),
    message(`${ATTACHMENTS_ID}-m2`, 'assistant', [
      { type: 'text', text: 'The chart shows four bars; the report summarizes Q3 results.' },
    ]),
  ],
  branches: [],
}

/** Deep bundles keyed by conversation id — merged into the chat cassette so the
 *  gallery renders each via the REAL Conversation.get / Message.getHistory path. */
export const chatDeepById: Record<string, DeepBundle> = {
  [TOOL_RUNNING_ID]: toolRunning,
  [TOOL_FAILED_ID]: toolFailed,
  [ATTACHMENTS_ID]: attachments,
}

export const CHAT_DEEP_CONVERSATION_IDS = {
  toolRunning: TOOL_RUNNING_ID,
  toolFailed: TOOL_FAILED_ID,
  attachments: ATTACHMENTS_ID,
} as const

// ── transient-state seeds (driven through the real store by deepStates.tsx) ──

/**
 * Recorded SSE-stream frames for the STREAMING state, replayed serverlessly.
 * Each frame is the exact object the per-user chat-token stream delivers to
 * `Chat.applyStreamFrame(conversationId, event)` — a `started` frame then a run
 * of `content` text-deltas — so replaying them drives the genuine streaming
 * reducer (placeholder message → streamingMessage → live text). We stop before
 * `complete`, leaving the conversation visibly mid-generation (`isStreaming`).
 */
export interface StreamFrame {
  type: 'started' | 'content' | 'complete' | 'error'
  message_id?: string
  user_message_id?: string
  content?: Array<{ type: 'text_delta'; delta: string }>
}
export const STREAMING_MESSAGE_ID = 'dee90009-0000-4000-8000-00000000str1'
export const streamingCassette: StreamFrame[] = [
  { type: 'started', message_id: STREAMING_MESSAGE_ID },
  { type: 'content', message_id: STREAMING_MESSAGE_ID, content: [{ type: 'text_delta', delta: 'Here is a live ' }] },
  { type: 'content', message_id: STREAMING_MESSAGE_ID, content: [{ type: 'text_delta', delta: 'streaming response ' }] },
  { type: 'content', message_id: STREAMING_MESSAGE_ID, content: [{ type: 'text_delta', delta: 'assembling token by ' }] },
  { type: 'content', message_id: STREAMING_MESSAGE_ID, content: [{ type: 'text_delta', delta: 'token as the model ' }] },
  { type: 'content', message_id: STREAMING_MESSAGE_ID, content: [{ type: 'text_delta', delta: 'generates it…' }] },
]

/**
 * A PENDING elicitation matching the showcase conversation's `elicitation_request`
 * block (elicitation_id `elic-0001`) — seeding this into the McpComposer store
 * flips its `elicitation_request` renderer from a static history block into the
 * live, answerable form.
 */
export const pendingElicitation: SSEChatStreamMcpElicitationRequiredData = {
  elicitation_id: 'elic-0001',
  message: 'Which output format do you want for the export?',
  server: 'Code Sandbox',
  requested_schema: {
    type: 'object',
    properties: {
      format: { type: 'string', enum: ['csv', 'json', 'xlsx'], title: 'Format' },
    },
    required: ['format'],
  },
}

/** A file to seed into the File store so the right-panel file viewer resolves. */
export const rightPanelFile: FileEntity = {
  id: 'f1000000-0000-0000-0000-000000000005',
  filename: 'report.pdf',
  mime_type: 'application/pdf',
  file_size: 631,
  blob_version_id: 'fv100000-0000-0000-0000-000000000005',
  current_version_id: 'fv100000-0000-0000-0000-000000000005',
  version: 1,
  has_thumbnail: false,
  preview_page_count: 1,
  text_page_count: 1,
  processing_metadata: {},
  created_by: 'user',
  user_id: 'aaaa0000-0000-0000-0000-000000000001',
  created_at: NOW,
  updated_at: NOW,
}

/** Right-panel literature screening payload (a couple of records, undecided). */
export const literaturePanelData: LiteratureScreeningData = {
  sessionId: 'lit-gallery-0001',
  query: 'CRISPR base editing off-target',
  records: [
    {
      doi: '10.1000/demo.1',
      title: 'Base editing minimizes off-target effects in primary cells',
      authors: ['A. Researcher', 'B. Scientist'],
      year: 2025,
      venue: 'Nature Methods',
      source: 'europepmc',
      source_ids: ['PMC1000001'],
      is_preprint: false,
      relevance: 0.92,
    },
    {
      doi: '10.1000/demo.2',
      title: 'A survey of prime editing delivery vectors',
      authors: ['C. Author'],
      year: 2024,
      venue: 'Cell',
      source: 'crossref',
      source_ids: [],
      is_preprint: false,
      relevance: 0.71,
    },
  ],
  identified: { europepmc: 1, crossref: 1 },
  afterDedup: 2,
  degradedSources: [],
  decisions: {},
  reasons: {},
}
