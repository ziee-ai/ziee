import { createModule } from '@/core'
import { PlusOutlined, HistoryOutlined } from '@ant-design/icons'
import AppLayout from '@/components/Layout/AppLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const ChatPage = lazyWithPreload(() => import('./ChatPage.tsx'))

export default createModule({
  metadata: {
    name: 'chat',
    version: '1.0.0',
    description: 'Chat module for conversations',
  },
  routes: [
    {
      path: '/chat',
      element: ChatPage,
      requiresAuth: true,
      layout: AppLayout,
    },
  ],
  sidebar: {
    primaryActions: [
      {
        id: 'new-chat',
        icon: <PlusOutlined />,
        label: 'New Chat',
        to: '/chat',
        order: 10,
      },
    ],
    navigation: [
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
