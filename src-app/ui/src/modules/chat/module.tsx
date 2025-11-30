import { createModule } from '@/core'
import { PlusOutlined, HistoryOutlined } from '@ant-design/icons'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useChatStore } from './core/stores/Chat.store'
import { useChatHistoryStore } from './stores/ChatHistory.store'
import { RecentConversationsWidget } from './widgets/RecentConversationsWidget'
import './types'
import './core/events' // Import chat events for type merging
import './extensions' // Auto-discover and register chat extensions

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
        icon: <PlusOutlined />,
        label: 'New Chat',
        to: '/chat',
        order: 10,
      },
    ],
    sidebarNavigation: [
      {
        id: 'chats',
        icon: <HistoryOutlined />,
        label: 'Chats',
        path: '/chats',
        order: 10,
      },
    ],
    sidebarContent: [
      {
        id: 'recent-conversations',
        component: RecentConversationsWidget,
        order: 10,
      },
    ],
  },
  initialize: () => {
    console.log('Chat module initialized')
  },
})
