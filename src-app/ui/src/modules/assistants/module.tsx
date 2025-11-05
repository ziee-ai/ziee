import { createModule } from '@/core'
import { UserOutlined } from '@ant-design/icons'
import AssistantsPage from './AssistantsPage'
import AppLayout from '@/components/Layout/AppLayout'

export default createModule({
  metadata: {
    name: 'assistants',
    version: '1.0.0',
    description: 'AI Assistants module',
  },
  routes: [
    {
      path: '/assistants',
      element: <AssistantsPage />,
      requiresAuth: true,
      layout: AppLayout,
    },
  ],
  sidebar: {
    tools: [
      {
        id: 'assistants',
        icon: <UserOutlined />,
        label: 'Assistants',
        path: '/assistants',
        order: 20,
      },
    ],
  },
  initialize: () => {
    console.log('Assistants module initialized')
  },
})
