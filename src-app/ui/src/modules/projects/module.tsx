import { createModule } from '@/core'
import { FolderOutlined, FolderAddOutlined } from '@ant-design/icons'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const ProjectsPage = lazyWithPreload(() => import('./ProjectsPage'))

export default createModule({
  metadata: {
    name: 'projects',
    version: '1.0.0',
    description: 'Projects module',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/projects',
      element: ProjectsPage,
      requiresAuth: true,
      layout: AppLayoutDef,
    },
  ],
  slots: {
    sidebarPrimaryActions: [
      {
        id: 'new-project',
        icon: <FolderAddOutlined />,
        label: 'New Project',
        to: '/projects/new',
        order: 20,
      },
    ],
    sidebarNavigation: [
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
