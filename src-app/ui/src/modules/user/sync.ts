import { registerSync } from '@/core/sync'
import { useUserGroupsStore } from '@/modules/user/stores/UserGroups.store'
import { useUsersStore } from '@/modules/user/stores/Users.store'

// Admin users + groups tables (both paginated). `load*` reloads the
// current page (it only skips while a load is already in flight), which
// reflects remote updates/deletes of rows on the current page. A remote
// create shows on the next page navigation rather than yanking the view.
registerSync('user', {
  onEvent: () => {
    void useUsersStore.getState().loadUsers()
  },
  onResync: () => {
    void useUsersStore.getState().loadUsers()
  },
})

registerSync('group', {
  onEvent: () => {
    void useUserGroupsStore.getState().loadUserGroups()
  },
  onResync: () => {
    void useUserGroupsStore.getState().loadUserGroups()
  },
})
