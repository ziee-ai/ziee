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

export const chatConversations = fixture.conversations
export const chatById = fixture.byId
/** Showcase conversation ids — each a distinct chat-detail gallery combo. */
export const showcaseConversationIds = Object.keys(fixture.byId)
const firstId = showcaseConversationIds[0]

export const chatCassette: Cassette = {
  'Conversation.list': chatConversations,
  'Conversation.get': ({ params }) =>
    (chatById[params.id] ?? chatById[firstId])?.conversation,
  'Message.getHistory': ({ params }) =>
    (chatById[params.id] ?? chatById[firstId])?.messages ?? [],
  'Branch.list': ({ params }) =>
    (chatById[params.id] ?? chatById[firstId])?.branches ?? [],
}
