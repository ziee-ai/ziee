import { createModule } from '@/core'
import { PlusOutlined, HistoryOutlined } from '@ant-design/icons'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useChatLlmProviderStore } from './core/stores/LlmProvider.store'
import { useChatStore } from './core/stores/Chat.store'
import './types'
import './extensions' // Auto-discover and register chat extensions

const NewChatPage = lazyWithPreload(() => import('./pages/NewChatPage'))
const ConversationPage = lazyWithPreload(
  () => import('./pages/ConversationPage'),
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
      name: 'ChatLlmProvider',
      store: useChatLlmProviderStore,
    },
    {
      name: 'Chat',
      store: useChatStore,
    },
  ],
  routes: [
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
        path: '/chat',
        order: 10,
      },
    ],
  },
  initialize: () => {
    console.log('Chat module initialized')
  },
})
