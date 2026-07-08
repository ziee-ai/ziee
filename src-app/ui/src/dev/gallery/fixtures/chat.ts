/**
 * Chat fixture — recorded conversation-detail endpoints for the showcase
 * conversations (rich / tool-calls / branched / attachments). Each is keyed by
 * conversationId so the gallery can render distinct conversation states in
 * ISOLATION (one full page reload per combo → the single-active `Chat` singleton
 * never bleeds across entries — see the isolation policy in SEEDED_GALLERY_PLAN).
 */
import type {
  Branch,
  Conversation,
  ConversationListResponse,
  MessageWithContent,
} from '@/api-client/types'
import type { Cassette } from '../mockApi'
import recorded from './recorded/chat.json'

interface ChatConversationBundle {
  conversation: Conversation
  messages: MessageWithContent[]
  branches: Branch[]
}
interface ChatFixture {
  conversations: ConversationListResponse
  byId: Record<string, ChatConversationBundle>
}

const fixture: ChatFixture = recorded as ChatFixture

/** Showcase conversation ids — each a distinct chat-detail gallery combo.
 *  Derived from the recorded fixture ALONE (chat-deep.ts imports this, so it must
 *  not depend on the deep bundles — no import cycle). */
export const showcaseConversationIds = Object.keys(fixture.byId)
const firstId = showcaseConversationIds[0]

// Merge the synthetic deep-state bundles (tool running/failed, attachments) in.
// Imported AFTER `showcaseConversationIds` is defined so the cycle stays acyclic
// (chat-deep only reads the exported id list, not the merged map).
import { chatDeepById } from './chat-deep'

export const chatById: Record<string, ChatConversationBundle> = {
  ...fixture.byId,
  ...(chatDeepById as Record<string, ChatConversationBundle>),
}

// The list stays the RECORDED list (its rows are the richer `ConversationResponse`
// shape); the synthetic deep conversations are rendered by PINNED id via the
// isolated deep-state entries, so they don't need a list row.
export const chatConversations = fixture.conversations

export const chatCassette: Cassette = {
  'Conversation.list': chatConversations,
  'Conversation.get': ({ params }) =>
    (chatById[params.id] ?? chatById[firstId])?.conversation,
  // Paginated envelope. The gallery fixtures are small, so a single page holds
  // the whole conversation (no lazy-load boundaries to exercise here).
  'Message.getHistory': ({ params }) => ({
    messages: (chatById[params.id] ?? chatById[firstId])?.messages ?? [],
    has_more_before: false,
    has_more_after: false,
  }),
  // In-conversation search over the fixture messages (paginated), so the find
  // bar's results-list state renders in the gallery.
  'Message.searchInConversation': ({ params }) => {
    const bundle = chatById[params.id] ?? chatById[firstId]
    const term = String(params.q ?? '').trim().toLowerCase()
    const perPage = Number(params.per_page ?? 25)
    const page = Number(params.page ?? 1)
    const all = term
      ? (bundle?.messages ?? []).filter(m =>
          m.contents.some(
            c =>
              c.content_type === 'text' &&
              String((c.content as { text?: string } | null)?.text ?? '')
                .toLowerCase()
                .includes(term),
          ),
        )
      : []
    const start = (page - 1) * perPage
    const slice = all.slice(start, start + perPage)
    return {
      matches: slice.map((m, i) => ({
        message_id: m.id,
        role: m.role,
        created_at: m.created_at,
        snippet:
          String(
            (m.contents.find(c => c.content_type === 'text')?.content as
              | { text?: string }
              | null)?.text ?? '',
          ).slice(0, 160),
        ordinal: start + i + 1,
      })),
      total: all.length,
      page,
      per_page: perPage,
    }
  },
  'Branch.list': ({ params }) =>
    (chatById[params.id] ?? chatById[firstId])?.branches ?? [],
}
