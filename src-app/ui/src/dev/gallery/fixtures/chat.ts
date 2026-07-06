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
  'Message.getHistory': ({ params }) =>
    (chatById[params.id] ?? chatById[firstId])?.messages ?? [],
  'Branch.list': ({ params }) =>
    (chatById[params.id] ?? chatById[firstId])?.branches ?? [],
}
