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
        // `tool_use_id` links this result to its `tool_use` block above — without
        // it the historical `McpToolUseRenderer` can't pair them, so it falls back
        // to the neutral wrench instead of the is_error red X.
        tool_use_id: 'toolu_failed_1',
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

// A DEDICATED short conversation ending in a PENDING elicitation, so the live
// answerable form is front-and-center (the showcase's pending elicitation block
// is buried deep in its 47-message thread, and its `elic-0001` block is already
// `accepted`). The block's own `status: 'pending'` is what renders the form.
const ELICITATION_ID = 'dee90004-0000-4000-8000-000000000004'
export const LIVE_ELICITATION_ID = 'elic-live-0001'
const elicitation: DeepBundle = {
  conversation: conversation(ELICITATION_ID, 'Elicitation — awaiting input'),
  messages: [
    message(`${ELICITATION_ID}-m1`, 'user', [
      { type: 'text', text: 'Export the results — pick whatever format is best.' },
    ]),
    {
      // Built manually (elicitation_request isn't in the MessageContentData union;
      // the mcp extension likewise casts it) — an assistant turn whose single block
      // is a pending elicitation the ElicitationFormContent renderer turns into a form.
      id: `${ELICITATION_ID}-m2`,
      role: 'assistant',
      contents: [
        {
          id: `${ELICITATION_ID}-m2-c0`,
          message_id: `${ELICITATION_ID}-m2`,
          content_type: 'elicitation_request',
          content: {
            type: 'elicitation_request',
            status: 'pending',
            elicitation_id: LIVE_ELICITATION_ID,
            message_id: `${ELICITATION_ID}-m2`,
            message: 'Which output format do you want for the export?',
            server: 'Code Sandbox',
            requested_schema: {
              type: 'object',
              properties: {
                format: {
                  type: 'string',
                  enum: ['csv', 'json', 'xlsx'],
                  title: 'Format',
                },
                include_headers: { type: 'boolean', title: 'Include headers' },
              },
              required: ['format'],
            },
          },
          sequence_order: 0,
          created_at: NOW,
          updated_at: NOW,
        } as unknown as MessageContent,
      ],
      originated_from_id: '',
      edit_count: 0,
      created_at: NOW,
      model_id: 'claude-opus-4-8',
    },
  ],
  branches: [],
}

/** The pending elicitation seeded into McpComposer so the form is also a LIVE
 *  entry (freshest status) — matches the dedicated bundle's block id. */
export const liveElicitation: SSEChatStreamMcpElicitationRequiredData = {
  elicitation_id: LIVE_ELICITATION_ID,
  message: 'Which output format do you want for the export?',
  server: 'Code Sandbox',
  requested_schema: {
    type: 'object',
    properties: {
      format: { type: 'string', enum: ['csv', 'json', 'xlsx'], title: 'Format' },
      include_headers: { type: 'boolean', title: 'Include headers' },
    },
    required: ['format'],
  },
}

// A DEDICATED conversation ending in a pending ziee-internal `ask_user`
// elicitation (marked `x-ziee-askuser`), so the RICH decision UX renders: a
// 2-question wizard of selectable option cards with per-option descriptions, a
// recommended-first badge, an inline preview, and the always-available Other
// escape. Distinct from the plain (external-MCP) elicitation cell above.
const ASKUSER_ID = 'dee90007-0000-4000-8000-000000000007'
export const LIVE_ASKUSER_ID = 'askuser-live-0001'
/** The rich ask_user schema — shared by the history block + the live seed. */
const ASKUSER_SCHEMA = {
  'x-ziee-askuser': true,
  type: 'object',
  properties: {
    format: {
      type: 'string',
      title: 'Which output format do you want?',
      description: 'Pick the export format that fits your downstream tooling.',
      enum: ['csv', 'json', 'parquet'],
      enumNames: ['CSV', 'JSON', 'Parquet'],
      enumDescriptions: [
        'Spreadsheet-friendly, untyped, widest compatibility.',
        'Nested + typed, human-readable, larger files.',
        'Columnar + typed, compact, best for analytics.',
      ],
      enumPreviews: ['id,name\n1,Ann', '{ "id": 1 }', null],
      'x-ziee-recommended': 'parquet',
    },
    compression: {
      type: 'string',
      title: 'Which compression?',
      description: 'Trade file size against read speed.',
      enum: ['none', 'gzip', 'zstd'],
      enumNames: ['None', 'gzip', 'zstd'],
      enumDescriptions: [
        'Largest, fastest to read.',
        'Smaller, ubiquitous, slower.',
        'Smallest with fast reads.',
      ],
      'x-ziee-recommended': 'zstd',
    },
  },
  required: ['format', 'compression'],
}
const askUser: DeepBundle = {
  conversation: conversation(ASKUSER_ID, 'ask_user — pick export options'),
  messages: [
    message(`${ASKUSER_ID}-m1`, 'user', [
      { type: 'text', text: 'Export the results — I want to choose the options.' },
    ]),
    {
      id: `${ASKUSER_ID}-m2`,
      role: 'assistant',
      contents: [
        {
          id: `${ASKUSER_ID}-m2-c0`,
          message_id: `${ASKUSER_ID}-m2`,
          content_type: 'elicitation_request',
          content: {
            type: 'elicitation_request',
            status: 'pending',
            elicitation_id: LIVE_ASKUSER_ID,
            message_id: `${ASKUSER_ID}-m2`,
            message: 'A couple of quick choices for the export:',
            server: 'Assistant',
            requested_schema: ASKUSER_SCHEMA,
          },
          sequence_order: 0,
          created_at: NOW,
          updated_at: NOW,
        } as unknown as MessageContent,
      ],
      originated_from_id: '',
      edit_count: 0,
      created_at: NOW,
      model_id: 'claude-opus-4-8',
    },
  ],
  branches: [],
}

/** Live seed matching the ask_user bundle so the wizard is the freshest-status
 *  source (mirrors `liveElicitation`). */
export const liveAskUser: SSEChatStreamMcpElicitationRequiredData = {
  elicitation_id: LIVE_ASKUSER_ID,
  message: 'A couple of quick choices for the export:',
  server: 'Assistant',
  requested_schema: ASKUSER_SCHEMA,
}

// A DEDICATED short conversation whose last assistant message is a fork point, so
// the BranchNavigator (< 1 / 3 >) renders on a VISIBLE message. `forkPoints` is
// computed by `loadBranches` from an intricate parent/child branch graph; the
// deep-state seeds it DIRECTLY (a store field — the transient-seed pattern) so the
// navigator is deterministic without hand-crafting that graph.
const BRANCHED_ID = 'dee90005-0000-4000-8000-000000000005'
/** The three sibling branch ids for the branched surface (first = active). */
export const BRANCHED_BRANCH_IDS = [
  'dee90005-0000-4000-8000-0000000000b1',
  'dee90005-0000-4000-8000-0000000000b2',
  'dee90005-0000-4000-8000-0000000000b3',
]
/** The message id the BranchNavigator anchors to (the last assistant message). */
export const BRANCHED_ANCHOR_MESSAGE_ID = `${BRANCHED_ID}-m2`
const branched: DeepBundle = {
  conversation: {
    ...conversation(BRANCHED_ID, 'Branched — edit / regenerate'),
    active_branch_id: BRANCHED_BRANCH_IDS[0],
  },
  messages: [
    message(`${BRANCHED_ID}-m1`, 'user', [
      { type: 'text', text: 'Give me a one-sentence summary of the SELECT trial.' },
    ]),
    message(BRANCHED_ANCHOR_MESSAGE_ID, 'assistant', [
      {
        type: 'text',
        text: 'SELECT showed semaglutide cut major adverse cardiovascular events by 20% (HR 0.80, 95% CI 0.72–0.90) in patients with obesity and established CVD.',
      },
    ]),
  ],
  branches: [],
}

/** Deep bundles keyed by conversation id — merged into the chat cassette so the
 *  gallery renders each via the REAL Conversation.get / Message.getHistory path. */
// A RENDERING SHOWCASE: one assistant message whose markdown exercises every
// rich-content renderer — block + inline math (KaTeX), a mermaid diagram, a
// syntax-highlighted code fence (Shiki), and a pipe table. Feeds the Layer-1
// content-rendering detectors (L1 math, L2 mermaid, L3 highlight, L4 table): the
// audit reports whether each renders or degrades to raw text (and whether a
// failure is gallery-only, e.g. Shiki under a preview build, or a real app bug).
export const RENDERING_SHOWCASE_ID = 'dee90006-0000-4000-8000-000000000006'
const renderingShowcase: DeepBundle = {
  conversation: conversation(RENDERING_SHOWCASE_ID, 'Rendering showcase — math / mermaid / code'),
  messages: [
    message(`${RENDERING_SHOWCASE_ID}-m1`, 'user', [
      { type: 'text', text: 'Show me a block equation, a diagram, a code block, and a table.' },
    ]),
    message(`${RENDERING_SHOWCASE_ID}-m2`, 'assistant', [
      {
        type: 'text',
        text: [
          'Block math:',
          '',
          '$$E = mc^2$$',
          '',
          'Inline math: the hypotenuse satisfies $a^2 + b^2 = c^2$.',
          '',
          '```mermaid',
          'graph TD',
          '  A[Start] --> B{Decision}',
          '  B -->|yes| C[Done]',
          '  B -->|no| A',
          '```',
          '',
          '```python',
          'def greet(name: str) -> str:',
          '    return f"Hello, {name}!"',
          '```',
          '',
          // HTML block → the code⇄preview toggle (sandboxed-iframe render).
          // Renders CODE by default; the `html-preview` interaction flips it.
          '```html',
          '<!doctype html>',
          '<html><body style="font-family:sans-serif;padding:12px">',
          '  <h1>Sandboxed preview</h1>',
          '  <p>Rendered inside a strictly-sandboxed iframe.</p>',
          '  <button onclick="this.textContent=\'clicked\'">Click me</button>',
          '</body></html>',
          '```',
          '',
          '| Trial | HR | 95% CI |',
          '| --- | --- | --- |',
          '| SELECT | 0.80 | 0.72–0.90 |',
          '| LEADER | 0.87 | 0.78–0.97 |',
        ].join('\n'),
      },
    ]),
  ],
  branches: [],
}

export const chatDeepById: Record<string, DeepBundle> = {
  [TOOL_RUNNING_ID]: toolRunning,
  [TOOL_FAILED_ID]: toolFailed,
  [ATTACHMENTS_ID]: attachments,
  [ELICITATION_ID]: elicitation,
  [ASKUSER_ID]: askUser,
  [BRANCHED_ID]: branched,
  [RENDERING_SHOWCASE_ID]: renderingShowcase,
}

export const CHAT_DEEP_CONVERSATION_IDS = {
  toolRunning: TOOL_RUNNING_ID,
  toolFailed: TOOL_FAILED_ID,
  attachments: ATTACHMENTS_ID,
  elicitation: ELICITATION_ID,
  askUser: ASKUSER_ID,
  branched: BRANCHED_ID,
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
