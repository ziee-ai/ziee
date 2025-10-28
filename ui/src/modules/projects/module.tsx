import { createModule } from '@/core'
import { FolderOutlined, FolderAddOutlined } from '@ant-design/icons'
import ProjectsPage from './ProjectsPage'
import AppLayout from '@/components/Layout/AppLayout'

export default createModule({
  metadata: {
    name: 'projects',
    version: '1.0.0',
    description: 'Projects module',
  },
  routes: [
    {
      path: '/projects',
      element: <ProjectsPage />,
      requiresAuth: true,
      layout: AppLayout,
    },
  ],
  sidebar: {
    primaryActions: [
      {
        id: 'new-project',
        icon: <FolderAddOutlined />,
        label: 'New Project',
        to: '/projects/new',
        order: 20,
      },
    ],
    navigation: [
      {
        id: 'projects',
        icon: <FolderOutlined />,
        label: 'Projects',
        path: '/projects',
        order: 20,
      },
    ],
  },
  initialize: () => {
    console.log('Projects module initialized')
  },
})
