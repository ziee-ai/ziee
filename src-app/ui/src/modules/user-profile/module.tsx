import { createModule } from '@/core'
import { UserProfileWidget } from './UserProfileWidget'

export default createModule({
  metadata: {
    name: 'user-profile',
    version: '1.0.0',
    description: 'User profile widget in sidebar footer',
  },
  routes: [],
  sidebar: {
    widgets: [
      {
        id: 'user-profile',
        slot: 'footer',
        component: <UserProfileWidget />,
        order: 0,
      },
    ],
  },
  initialize: () => {
    console.log('User Profile module initialized')
  },
})
