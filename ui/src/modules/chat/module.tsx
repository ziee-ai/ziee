import { createModule } from '@/core'
import ChatPage from './ChatPage'

export default createModule({
  metadata: {
    name: 'chat',
    version: '1.0.0',
    description: 'Chat module for messaging',
  },
  routes: [
    {
      path: '/',
      element: <ChatPage />,
      requiresAuth: true,
      layout: 'default',
      index: true,
    },
    {
      path: '/chat',
      element: <ChatPage />,
      requiresAuth: true,
      layout: 'default',
    },
  ],
  initialize: () => {
    console.log('Chat module initialized')
  },
})
