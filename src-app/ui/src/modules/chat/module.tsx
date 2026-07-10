import { createModule } from '@/core'
import { Permissions } from '@/api-client/types'
import { History, Plus } from 'lucide-react'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useChatStore } from '@/modules/chat/core/stores/Chat.store'
import { useChatHistoryStore } from '@/modules/chat/stores/ChatHistory.store'
import { useMessageViewStateStore } from '@/modules/chat/core/stores/MessageViewState.store'
import { RecentConversationsWidget } from '@/modules/chat/widgets/RecentConversationsWidget'
import { OpenInNewWindowAction } from '@/modules/chat/components/OpenInNewWindowAction'
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
      },
    ],
    // Per-conversation header decoration: "Open in new window / tab".
    // order 30 = before the desktop host-mount control (order 40).
    chatConversationHeaderTrailing: [
      {
        id: 'chat-open-in-new-window',
        order: 30,
        component: OpenInNewWindowAction,
      },
    ],
  },
  initialize: () => {
    console.log('Chat module initialized')
  },
})
