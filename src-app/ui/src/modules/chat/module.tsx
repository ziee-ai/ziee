import { createModule } from '@/core'
import { Permissions } from '@/api-client/types'
import { History, Plus } from 'lucide-react'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useChatStore } from '@/modules/chat/core/stores/Chat.store'
import { useChatHistoryStore } from '@/modules/chat/stores/ChatHistory.store'
import { useMessageViewStateStore } from '@/modules/chat/core/stores/MessageViewState.store'
import { RecentConversationsWidget } from '@/modules/chat/widgets/RecentConversationsWidget'
import '@/modules/chat/types'
import '@/modules/chat/core/events' // Import chat events for type merging
import '@/modules/chat/extensions' // Auto-discover and register chat extensions

const NewChatPage = lazyWithPreload(() => import('./pages/NewChatPage'))
const ConversationPage = lazyWithPreload(
  () => import('./pages/ConversationPage'),
)
const ChatHistoryPage = lazyWithPreload(
  () => import('./pages/ChatHistoryPage'),
)

export default createModule({
  metadata: {
    name: 'chat',
    version: '1.0.0',
    description: 'Chat module for conversations',
  },
  dependencies: ['router'],
  stores: [
    {
      name: 'Chat',
      store: useChatStore,
    },
    {
      name: 'ChatHistory',
      store: useChatHistoryStore,
    },
    {
      name: 'MessageViewState',
      store: useMessageViewStateStore,
    },
  ],
  routes: [
    {
      path: '/',
      element: NewChatPage,
      requiresAuth: true,
      layout: AppLayoutDef,
    },
    {
      path: '/chat',
      element: NewChatPage,
      requiresAuth: true,
      layout: AppLayoutDef,
    },
    {
      path: '/chat/:conversationId',
      element: ConversationPage,
      requiresAuth: true,
      layout: AppLayoutDef,
    },
    {
      path: '/chats',
      element: ChatHistoryPage,
      requiresAuth: true,
      // Route gate MUST match the `chats` nav slot's ConversationsRead —
      // without it, a user lacking conversations::read had the menu item
      // hidden but could still deep-link /chats and render the full
      // conversation history (no 403). The base chat routes (/, /chat,
      // /chat/:id) stay ungated by design (new-chat is always available).
      permission: Permissions.ConversationsRead,
      layout: AppLayoutDef,
    },
  ],
  slots: {
    sidebarPrimaryActions: [
      {
        id: 'new-chat',
        icon: <Plus />,
        label: 'New Chat',
        to: '/chat',
        order: 10,
      },
    ],
    sidebarNavigation: [
      {
        id: 'chats',
        icon: <History />,
        label: 'Chats',
        path: '/chats',
        order: 10,
        permission: Permissions.ConversationsRead,
      },
    ],
    sidebarContent: [
      {
        id: 'recent-conversations',
        component: RecentConversationsWidget,
        order: 10,
        // Gate: this widget lists the user's conversations and fetches
        // them on mount (`Stores.ChatHistory.loadConversations()`). The
        // sibling `chats` nav entry is gated on ConversationsRead — match
        // it here so a user without the grant never sees the list nor
        // fires the 403 fetch.
        permission: Permissions.ConversationsRead,
      },
    ],
  },
  initialize: () => {
    console.log('Chat module initialized')
  },
})
